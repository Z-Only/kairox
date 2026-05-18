use super::streaming::OpenAiChunkEvent;
use crate::ModelEvent;
use std::collections::HashMap;

/// Accumulates streaming tool call arguments across SSE chunks.
///
/// OpenAI sends tool calls as:
/// 1. First chunk: `delta.tool_calls[i] = { id: "call_xxx", function: { name: "fs.read", arguments: "{\"pa" } }`
/// 2. Subsequent chunks: `delta.tool_calls[i] = { function: { arguments: "th\": \"R" } }`
/// 3. ... more argument chunks ...
/// 4. `finish_reason: "tool_calls"` signals completion
///
/// Only after all argument chunks have arrived do we have the complete JSON and
/// can emit a `ModelEvent::ToolCallRequested`.
pub(super) struct OpenAiToolCallAccumulator {
    /// Tool calls being accumulated, keyed by their index in the `tool_calls` array.
    pending: HashMap<usize, PendingOpenAiToolCall>,
}

struct PendingOpenAiToolCall {
    id: String,
    name: String,
    arguments_buffer: String,
}

impl OpenAiToolCallAccumulator {
    pub(super) fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Process a raw chunk event and return zero or more completed model events.
    pub(super) fn process(&mut self, raw: OpenAiChunkEvent) -> Vec<ModelEvent> {
        match raw {
            OpenAiChunkEvent::Event(e) => vec![e],
            OpenAiChunkEvent::ToolCallStarted { index, id, name } => {
                // If there was a previous tool call at this index (shouldn't happen
                // in normal streaming, but be safe), emit it before starting a new one.
                let mut events = Vec::new();
                if let Some(prev) = self.pending.remove(&index) {
                    events.push(self.finalize_pending(prev));
                }
                self.pending.insert(
                    index,
                    PendingOpenAiToolCall {
                        id,
                        name,
                        arguments_buffer: String::new(),
                    },
                );
                events
            }
            OpenAiChunkEvent::ToolCallArgumentDelta {
                index,
                partial_arguments,
            } => {
                if let Some(pending) = self.pending.get_mut(&index) {
                    pending.arguments_buffer.push_str(&partial_arguments);
                }
                vec![]
            }
        }
    }

    /// Finalize a pending tool call into a ModelEvent::ToolCallRequested.
    fn finalize_pending(&self, pending: PendingOpenAiToolCall) -> ModelEvent {
        let arguments: serde_json::Value =
            serde_json::from_str(&pending.arguments_buffer).unwrap_or(serde_json::json!({}));
        ModelEvent::ToolCallRequested {
            tool_call_id: pending.id,
            tool_id: pending.name,
            arguments,
        }
    }

    /// Flush all remaining pending tool calls into model events.
    /// Called when the stream ends (finish_reason = "tool_calls" or "stop").
    pub(super) fn flush(&mut self) -> Vec<ModelEvent> {
        let pending = std::mem::take(&mut self.pending);
        pending
            .into_values()
            .map(|p| self.finalize_pending(p))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_tool_call_across_chunks() {
        let mut acc = OpenAiToolCallAccumulator::new();

        let events = acc.process(OpenAiChunkEvent::Event(ModelEvent::TokenDelta(
            "Reading file...".into(),
        )));
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ModelEvent::TokenDelta(t) if t == "Reading file..."));

        let events = acc.process(OpenAiChunkEvent::ToolCallStarted {
            index: 0,
            id: "call_abc".into(),
            name: "fs.read".into(),
        });
        assert!(events.is_empty());

        let events = acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
            index: 0,
            partial_arguments: "{\"pa".into(),
        });
        assert!(events.is_empty());

        let events = acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
            index: 0,
            partial_arguments: "th\": \"README.md\"}".into(),
        });
        assert!(events.is_empty());

        let events = acc.process(OpenAiChunkEvent::Event(ModelEvent::Completed {
            usage: None,
        }));
        assert_eq!(events.len(), 1);

        let events = acc.flush();
        assert_eq!(events.len(), 1);
        match &events[0] {
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => {
                assert_eq!(tool_call_id, "call_abc");
                assert_eq!(tool_id, "fs.read");
                assert_eq!(arguments["path"], "README.md");
            }
            _ => panic!("expected ToolCallRequested"),
        }
    }

    #[test]
    fn handles_multiple_tool_calls() {
        let mut acc = OpenAiToolCallAccumulator::new();

        acc.process(OpenAiChunkEvent::ToolCallStarted {
            index: 0,
            id: "call_1".into(),
            name: "fs.read".into(),
        });
        acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
            index: 0,
            partial_arguments: "{\"path\":\"README.md\"}".into(),
        });

        acc.process(OpenAiChunkEvent::ToolCallStarted {
            index: 1,
            id: "call_2".into(),
            name: "shell.exec".into(),
        });
        acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
            index: 1,
            partial_arguments: "{\"command\":\"ls\"}".into(),
        });

        let mut events = acc.flush();
        assert_eq!(events.len(), 2);

        events.sort_by_key(|e| match e {
            ModelEvent::ToolCallRequested { tool_call_id, .. } => tool_call_id.clone(),
            _ => String::new(),
        });

        match &events[0] {
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(tool_id, "fs.read");
                assert_eq!(arguments["path"], "README.md");
            }
            _ => panic!("expected ToolCallRequested"),
        }
        match &events[1] {
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => {
                assert_eq!(tool_call_id, "call_2");
                assert_eq!(tool_id, "shell.exec");
                assert_eq!(arguments["command"], "ls");
            }
            _ => panic!("expected ToolCallRequested"),
        }
    }
}

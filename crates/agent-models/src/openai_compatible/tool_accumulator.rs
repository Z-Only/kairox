use super::streaming::OpenAiChunkEvent;
use super::tool_names::OpenAiToolNameMap;
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
    tool_name_map: OpenAiToolNameMap,
}

struct PendingOpenAiToolCall {
    id: String,
    name: String,
    arguments_buffer: String,
}

impl OpenAiToolCallAccumulator {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::with_tool_name_map(OpenAiToolNameMap::default())
    }

    pub(super) fn with_tool_name_map(tool_name_map: OpenAiToolNameMap) -> Self {
        Self {
            pending: HashMap::new(),
            tool_name_map,
        }
    }

    /// Process a raw chunk event and return zero or more completed model events.
    pub(super) fn process(&mut self, raw: OpenAiChunkEvent) -> Vec<ModelEvent> {
        match raw {
            OpenAiChunkEvent::Event(e) => {
                if matches!(e, ModelEvent::Completed { .. }) && !self.pending.is_empty() {
                    let mut events = self.flush();
                    events.push(e);
                    events
                } else {
                    vec![e]
                }
            }
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
            tool_id: self.tool_name_map.internal_name(&pending.name),
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
#[path = "tool_accumulator_tests.rs"]
mod tests;

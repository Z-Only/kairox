use super::streaming::AnthropicRawEvent;
use crate::{ModelEvent, ToolDefinition};
use std::collections::HashMap;

/// Accumulates tool call arguments across SSE chunks.
///
/// Anthropic sends tool calls as:
/// 1. `content_block_start` → { id, name }
/// 2. One or more `content_block_delta` → { partial_json } fragments
/// 3. `content_block_stop`
///
/// Only after step 3 do we have the complete arguments JSON and can emit
/// a `ModelEvent::ToolCallRequested`.
pub(super) struct AnthropicToolCallAccumulator {
    /// The tool_use block currently being accumulated, if any.
    pending: Option<PendingToolCall>,
    /// Map from Anthropic-safe names (e.g. "shell_exec") back to original
    /// names (e.g. "shell.exec"). Built from the tools sent in the request.
    name_map: HashMap<String, String>,
}

struct PendingToolCall {
    id: String,
    safe_name: String,
    arguments_buffer: String,
}

pub(super) fn anthropic_tool_name_map(tools: &[ToolDefinition]) -> HashMap<String, String> {
    tools
        .iter()
        .map(|tool| {
            let safe_name: String = tool
                .name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            (safe_name, tool.name.clone())
        })
        .collect()
}

impl AnthropicToolCallAccumulator {
    pub(super) fn new(name_map: HashMap<String, String>) -> Self {
        Self {
            pending: None,
            name_map,
        }
    }

    /// Flush any remaining pending tool calls into model events.
    /// Called when the stream ends to emit any tool calls that haven't
    /// been finalized by a content_block_stop event.
    pub(super) fn flush(&mut self) -> Vec<ModelEvent> {
        // For Anthropic, pending tool calls should normally be flushed by
        // ToolUseFinished events. If there's still a pending call at stream
        // end, emit it as a safety net.
        if let Some(pending) = self.pending.take() {
            let original_name = self
                .name_map
                .get(&pending.safe_name)
                .cloned()
                .unwrap_or(pending.safe_name);
            let arguments: serde_json::Value =
                serde_json::from_str(&pending.arguments_buffer).unwrap_or(serde_json::json!({}));
            vec![ModelEvent::ToolCallRequested {
                tool_call_id: pending.id,
                tool_id: original_name,
                arguments,
            }]
        } else {
            vec![]
        }
    }

    /// Process a raw event and return zero or more completed model events.
    pub(super) fn process(&mut self, raw: AnthropicRawEvent) -> Vec<ModelEvent> {
        match raw {
            AnthropicRawEvent::Event(e) => vec![e],
            AnthropicRawEvent::ToolUseStarted { id, name } => {
                self.pending = Some(PendingToolCall {
                    id,
                    safe_name: name,
                    arguments_buffer: String::new(),
                });
                vec![]
            }
            AnthropicRawEvent::ToolUseArgumentDelta { partial_json } => {
                if let Some(ref mut pending) = self.pending {
                    pending.arguments_buffer.push_str(&partial_json);
                }
                vec![]
            }
            AnthropicRawEvent::ToolUseFinished => {
                if let Some(pending) = self.pending.take() {
                    let original_name = self
                        .name_map
                        .get(&pending.safe_name)
                        .cloned()
                        .unwrap_or(pending.safe_name);
                    let arguments: serde_json::Value =
                        serde_json::from_str(&pending.arguments_buffer)
                            .unwrap_or(serde_json::json!({}));
                    vec![ModelEvent::ToolCallRequested {
                        tool_call_id: pending.id,
                        tool_id: original_name,
                        arguments,
                    }]
                } else {
                    vec![]
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_safe_to_original_tool_name_map() {
        let tools = vec![ToolDefinition {
            name: "shell.exec".into(),
            description: "Execute a shell command".into(),
            parameters: serde_json::json!({"type": "object"}),
        }];

        let name_map = anthropic_tool_name_map(&tools);

        assert_eq!(name_map.get("shell_exec"), Some(&"shell.exec".to_string()));
    }

    #[test]
    fn accumulates_tool_call_across_chunks() {
        let name_map = HashMap::from([
            ("shell_exec".to_string(), "shell.exec".to_string()),
            ("fs_read".to_string(), "fs.read".to_string()),
        ]);
        let mut acc = AnthropicToolCallAccumulator::new(name_map);

        let events = acc.process(AnthropicRawEvent::Event(ModelEvent::TokenDelta(
            "I'll list files.".into(),
        )));
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ModelEvent::TokenDelta(t) if t == "I'll list files."));

        let events = acc.process(AnthropicRawEvent::ToolUseStarted {
            id: "toolu_01".into(),
            name: "shell_exec".into(),
        });
        assert!(events.is_empty());

        let events = acc.process(AnthropicRawEvent::ToolUseArgumentDelta {
            partial_json: "{\"command\":".into(),
        });
        assert!(events.is_empty());
        let events = acc.process(AnthropicRawEvent::ToolUseArgumentDelta {
            partial_json: " \"ls\"}".into(),
        });
        assert!(events.is_empty());

        let events = acc.process(AnthropicRawEvent::ToolUseFinished);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => {
                assert_eq!(tool_call_id, "toolu_01");
                assert_eq!(tool_id, "shell.exec");
                assert_eq!(arguments["command"], "ls");
            }
            _ => panic!("expected ToolCallRequested"),
        }

        let events = acc.process(AnthropicRawEvent::ToolUseFinished);
        assert!(events.is_empty());
    }

    #[test]
    fn handles_unknown_tool_name() {
        let mut acc = AnthropicToolCallAccumulator::new(HashMap::new());

        acc.process(AnthropicRawEvent::ToolUseStarted {
            id: "toolu_02".into(),
            name: "custom_tool".into(),
        });
        acc.process(AnthropicRawEvent::ToolUseArgumentDelta {
            partial_json: "{}".into(),
        });
        let events = acc.process(AnthropicRawEvent::ToolUseFinished);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ModelEvent::ToolCallRequested { tool_id, .. } => {
                assert_eq!(tool_id, "custom_tool");
            }
            _ => panic!("expected ToolCallRequested"),
        }
    }
}

use super::streaming::AnthropicRawEvent;
use crate::ModelEvent;
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

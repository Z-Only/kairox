use super::AnthropicClient;
use crate::ModelRequest;

impl AnthropicClient {
    pub(super) fn build_messages_request(&self, request: &ModelRequest) -> serde_json::Value {
        let mut messages = Vec::new();

        // Anthropic Messages API: system prompt is a top-level field, not a message.
        // Tool results (role="tool") must be in user messages with tool_result content blocks.
        // Assistant messages with tool calls must include tool_use content blocks.
        for msg in &request.messages {
            if msg.role == "tool" {
                // Tool result message - convert to Anthropic's tool_result content block.
                // Use tool_call_id from the message if available (preferred),
                // otherwise fall back to parsing the legacy format from content.
                let tool_use_id = msg.tool_call_id.clone().unwrap_or_else(|| {
                    msg.content
                        .lines()
                        .find(|l| l.starts_with("tool_call_id="))
                        .map(|l| l.trim_start_matches("tool_call_id=").to_string())
                        .unwrap_or_default()
                });

                // Extract result text
                let result_text = if msg.tool_call_id.is_some() {
                    // New format: content is plain text (tool_call_id is stored separately)
                    msg.content.clone()
                } else {
                    // Legacy format: "tool_call_id=X\ntool_id=Y\nresult=Z"
                    msg.content
                        .lines()
                        .find(|l| l.starts_with("result="))
                        .map(|l| l.trim_start_matches("result=").to_string())
                        .unwrap_or_else(|| msg.content.clone())
                };

                messages.push(serde_json::json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": result_text,
                    }]
                }));
            } else if msg.role == "assistant" {
                // Assistant message - may include tool_use content blocks.
                // Anthropic requires that if the next message contains tool_result
                // blocks, this assistant message MUST include the corresponding
                // tool_use blocks.
                let mut content_blocks: Vec<serde_json::Value> = Vec::new();

                // Add text content if present
                if !msg.content.is_empty() {
                    content_blocks.push(serde_json::json!({
                        "type": "text",
                        "text": msg.content,
                    }));
                }

                // Add tool_use blocks for each tool call
                for tc in &msg.tool_calls {
                    let safe_name: String = tc
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
                    content_blocks.push(serde_json::json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": safe_name,
                        "input": tc.arguments,
                    }));
                }

                // Anthropic requires at least one content block in an assistant message.
                if content_blocks.is_empty() {
                    content_blocks.push(serde_json::json!({
                        "type": "text",
                        "text": "",
                    }));
                }

                messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": content_blocks,
                }));
            } else {
                messages.push(serde_json::json!({
                    "role": msg.role,
                    "content": msg.content,
                }));
            }
        }

        let mut body = serde_json::json!({
            "model": self.config.default_model,
            "max_tokens": self.config.max_tokens,
            "messages": messages,
            "stream": true,
        });

        if let Some(ref system_prompt) = request.system_prompt {
            body["system"] = serde_json::json!(system_prompt);
        }

        // Tool definitions - map to Anthropic tool format if present.
        // Anthropic tool names must match ^[a-zA-Z0-9_-]{1,128}$,
        // so we replace dots and other invalid chars with underscores.
        if !request.tools.is_empty() {
            let tools: Vec<_> = request
                .tools
                .iter()
                .map(|t| {
                    let safe_name: String = t
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
                    serde_json::json!({
                        "name": safe_name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tools);
        }

        if let Some(temperature) = self.config.temperature {
            body["temperature"] = serde_json::json!(temperature);
        }
        if let Some(top_p) = self.config.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(top_k) = self.config.top_k {
            body["top_k"] = serde_json::json!(top_k);
        }
        if let Some(ref extra) = self.config.extra_params {
            if let Some(obj) = extra.as_object() {
                for (key, value) in obj {
                    body[key] = value.clone();
                }
            }
        }

        body
    }
}

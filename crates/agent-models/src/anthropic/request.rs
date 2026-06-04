use super::AnthropicClient;
use crate::content_parts::{split_markdown_data_uri_images, MultimodalContentPart};
use crate::types::ServerTool;
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
                if has_non_empty_text(&msg.content) {
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

                // Empty assistant messages with no tool_use blocks carry no
                // state and Anthropic rejects empty text blocks.
                if content_blocks.is_empty() {
                    continue;
                }

                messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": content_blocks,
                }));
            } else {
                if !has_non_empty_text(&msg.content) {
                    continue;
                }
                if msg.role == "user" {
                    if let Some(content_blocks) = anthropic_multimodal_content(&msg.content) {
                        messages.push(serde_json::json!({
                            "role": msg.role,
                            "content": content_blocks,
                        }));
                        continue;
                    }
                }
                messages.push(serde_json::json!({
                    "role": msg.role,
                    "content": msg.content,
                }));
            }
        }

        // Add cache_control breakpoints to the last N (up to 3) tool_result
        // messages so Anthropic can cache conversation prefixes up to those points.
        Self::add_tool_result_cache_breakpoints(&mut messages);

        let mut body = serde_json::json!({
            "model": self.config.default_model,
            "max_tokens": self.config.max_tokens,
            "messages": messages,
            "stream": true,
        });

        if let Some(system_prompt) = request
            .system_prompt
            .as_ref()
            .filter(|prompt| has_non_empty_text(prompt))
        {
            // Use content-block array format with cache_control on the last block
            // so Anthropic can cache the system prompt across turns.
            body["system"] = serde_json::json!([{
                "type": "text",
                "text": system_prompt,
                "cache_control": {"type": "ephemeral"},
            }]);
        }

        // Tool definitions - map to Anthropic tool format if present.
        // Anthropic tool names must match ^[a-zA-Z0-9_-]{1,128}$,
        // so we replace dots and other invalid chars with underscores.
        // Server-side tools are appended to the same array.
        if !request.tools.is_empty() || !request.server_tools.is_empty() {
            let mut tools: Vec<serde_json::Value> = request
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

            // Append server-side tools (code_execution, web_search)
            for st in &request.server_tools {
                tools.push(serialize_server_tool(st));
            }

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

        if let Some(budget_tokens) = request
            .reasoning_effort
            .as_deref()
            .and_then(|effort| reasoning_budget_tokens(effort, self.config.max_tokens))
        {
            if body["thinking"].is_null() {
                body["thinking"] = serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": budget_tokens,
                });
            }
        }

        body
    }

    /// Add `cache_control: {"type": "ephemeral"}` to the last N (up to 3)
    /// tool_result content blocks in the messages array.
    fn add_tool_result_cache_breakpoints(messages: &mut [serde_json::Value]) {
        const MAX_BREAKPOINTS: usize = 3;

        // Collect indices of messages that contain tool_result content blocks.
        let tool_result_indices: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter(|(_, msg)| {
                msg["content"]
                    .as_array()
                    .is_some_and(|blocks| blocks.iter().any(|b| b["type"] == "tool_result"))
            })
            .map(|(i, _)| i)
            .collect();

        // Take the last N indices
        let start = tool_result_indices.len().saturating_sub(MAX_BREAKPOINTS);
        for &idx in &tool_result_indices[start..] {
            if let Some(blocks) = messages[idx]["content"].as_array_mut() {
                // Add cache_control to the last tool_result block in this message
                if let Some(last_tr) = blocks.iter_mut().rev().find(|b| b["type"] == "tool_result")
                {
                    last_tr["cache_control"] = serde_json::json!({"type": "ephemeral"});
                }
            }
        }
    }
}

fn has_non_empty_text(value: &str) -> bool {
    !value.trim().is_empty()
}

fn anthropic_multimodal_content(content: &str) -> Option<Vec<serde_json::Value>> {
    split_markdown_data_uri_images(content).map(|parts| {
        parts
            .into_iter()
            .map(|part| match part {
                MultimodalContentPart::Text(text) => serde_json::json!({
                    "type": "text",
                    "text": text,
                }),
                MultimodalContentPart::Image {
                    mime_type, data, ..
                } => serde_json::json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": mime_type,
                        "data": data,
                    },
                }),
            })
            .collect()
    })
}

fn serialize_server_tool(st: &ServerTool) -> serde_json::Value {
    match st {
        ServerTool::CodeExecution => {
            serde_json::json!({
                "type": "code_execution_20250522",
                "name": "code_execution",
            })
        }
        ServerTool::WebSearch {
            allowed_domains,
            blocked_domains,
            user_location,
        } => {
            let mut tool = serde_json::json!({
                "type": "web_search_20250305",
                "name": "web_search",
            });
            if !allowed_domains.is_empty() {
                tool["allowed_domains"] = serde_json::json!(allowed_domains);
            }
            if !blocked_domains.is_empty() {
                tool["blocked_domains"] = serde_json::json!(blocked_domains);
            }
            if let Some(loc) = user_location {
                let mut loc_obj = serde_json::Map::new();
                if let Some(ref city) = loc.city {
                    loc_obj.insert("city".into(), serde_json::json!(city));
                }
                if let Some(ref region) = loc.region {
                    loc_obj.insert("region".into(), serde_json::json!(region));
                }
                if let Some(ref country) = loc.country {
                    loc_obj.insert("country".into(), serde_json::json!(country));
                }
                if let Some(ref timezone) = loc.timezone {
                    loc_obj.insert("timezone".into(), serde_json::json!(timezone));
                }
                tool["user_location"] = serde_json::Value::Object(loc_obj);
            }
            tool
        }
    }
}

fn reasoning_budget_tokens(effort: &str, max_tokens: u64) -> Option<u64> {
    if max_tokens <= 1_024 {
        return None;
    }

    let requested = match effort.trim().to_ascii_lowercase().as_str() {
        "" => return None,
        "low" => 1_024,
        "middle" | "medium" => 4_096,
        "high" => 8_192,
        "xhigh" | "extra-high" | "extra_high" => 16_384,
        custom => custom.parse().ok()?,
    };

    Some(requested.clamp(1_024, max_tokens.saturating_sub(1)))
}

use super::OpenAiCompatibleClient;
use crate::{ModelRequest, Result};

impl OpenAiCompatibleClient {
    pub(super) fn build_chat_request(&self, request: &ModelRequest) -> Result<serde_json::Value> {
        let mut messages = Vec::new();

        if let Some(system_prompt) = request
            .system_prompt
            .as_ref()
            .filter(|prompt| has_non_empty_text(prompt))
        {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system_prompt,
            }));
        }

        for msg in &request.messages {
            if msg.role == "assistant" && !msg.tool_calls.is_empty() {
                // Assistant message with tool calls — include tool_calls array
                // in OpenAI format so the API can match tool results to their calls.
                let tool_calls_json: Vec<serde_json::Value> = msg
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        serde_json::json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": tc.arguments.to_string(),
                            }
                        })
                    })
                    .collect();
                let mut msg_json = serde_json::json!({
                    "role": "assistant",
                    "content": if has_non_empty_text(&msg.content) { serde_json::json!(msg.content) } else { serde_json::Value::Null },
                });
                msg_json["tool_calls"] = serde_json::json!(tool_calls_json);
                messages.push(msg_json);
            } else if msg.role == "tool" {
                // Tool result message — include tool_call_id for OpenAI format.
                let tool_call_id = msg.tool_call_id.as_deref().unwrap_or("");
                messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": msg.content,
                }));
            } else {
                if !has_non_empty_text(&msg.content) {
                    continue;
                }
                messages.push(serde_json::json!({
                    "role": msg.role,
                    "content": msg.content,
                }));
            }
        }

        let mut body = serde_json::json!({
            "model": self.config.default_model,
            "messages": messages,
            "stream": true,
        });

        if !request.tools.is_empty() {
            let tools: Vec<_> = request
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
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
        if let Some(ref effort) = request.reasoning_effort {
            body["reasoning_effort"] = serde_json::json!(effort);
        }
        if let Some(ref extra) = self.config.extra_params {
            if let Some(obj) = extra.as_object() {
                for (key, value) in obj {
                    body[key] = value.clone();
                }
            }
        }

        Ok(body)
    }
}

fn has_non_empty_text(value: &str) -> bool {
    !value.trim().is_empty()
}

//! Anthropic Messages API client.
//!
//! Supports the `/v1/messages` endpoint with SSE streaming,
//! authenticating via the `x-api-key` header.

use crate::{ModelError, ModelEvent, ModelRequest, Result};
use async_trait::async_trait;
use futures::stream::BoxStream;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub base_url: String,
    pub api_key_env: String,
    pub default_model: String,
    pub max_tokens: u64,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub capability_overrides: Option<crate::ModelCapabilities>,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.anthropic.com".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            default_model: "claude-sonnet-4-20250514".into(),
            max_tokens: 16_384,
            headers: Vec::new(),
            capability_overrides: None,
        }
    }
}

impl AnthropicConfig {
    pub fn capabilities(&self) -> crate::ModelCapabilities {
        self.capability_overrides
            .clone()
            .unwrap_or(crate::ModelCapabilities {
                streaming: true,
                tool_calling: true,
                json_schema: false,
                vision: false,
                reasoning_controls: false,
                context_window: 200_000,
                output_limit: self.max_tokens,
                local_model: false,
            })
    }

    fn api_key(&self) -> Option<String> {
        // Direct key from env named in api_key_env
        std::env::var(&self.api_key_env).ok()
    }
}

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    config: AnthropicConfig,
    http: Client,
}

impl AnthropicClient {
    pub fn new(config: AnthropicConfig) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build reqwest client");
        Self { config, http }
    }

    fn build_messages_request(&self, request: &ModelRequest) -> serde_json::Value {
        let mut messages = Vec::new();

        // Anthropic Messages API: system prompt is a top-level field, not a message
        for msg in &request.messages {
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            }));
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

        // Tool definitions — map to Anthropic tool format if present
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

        body
    }

    async fn send_streaming(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        let body = self.build_messages_request(&request);
        let url = format!("{}/v1/messages", self.config.base_url.trim_end_matches('/'));

        let api_key = self
            .config
            .api_key()
            .ok_or_else(|| ModelError::Request("Anthropic API key not set".into()))?;

        let mut builder = self
            .http
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body);

        for (key, value) in &self.config.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }

        let response = builder
            .send()
            .await
            .map_err(|e| ModelError::Http(e.to_string()))?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();

            return Err(ModelError::Api(format!("HTTP {}: {}", status, body)));
        }

        // Read the full response body and parse SSE events from it.
        // This avoids issues with bytes_stream() + eventsource_stream
        // not producing events in certain async runtime contexts (e.g., Tauri).
        // The trade-off is that tokens arrive as a batch rather than streamed,
        // but this is more reliable across different runtime environments.
        let body_bytes = response
            .bytes()
            .await
            .map_err(|e| ModelError::Http(e.to_string()))?;
        let body_text = String::from_utf8_lossy(&body_bytes);

        // Debug: write response info to log file
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/kairox-debug.log")
        {
            use std::io::Write;
            let _ = writeln!(f, "[anthropic] status={} body_len={} starts_with_event={} starts_with_data={} starts_with_brace={}", 
                status, body_text.len(),
                body_text.trim().starts_with("event:"),
                body_text.trim().starts_with("data:"),
                body_text.trim().starts_with('{'));
            let _ = writeln!(
                f,
                "[anthropic] body_preview: {}",
                &body_text[..body_text.len().min(500)]
            );
        }

        // Detect response format: SSE streaming vs non-streaming JSON
        let trimmed = body_text.trim();

        let mut events: Vec<Result<ModelEvent>> = Vec::new();

        if trimmed.starts_with("event:") || trimmed.starts_with("data:") {
            // SSE streaming format - parse event blocks
            for block in trimmed.split("\n\n") {
                let mut data_line = "";
                for line in block.lines() {
                    if let Some(rest) = line.strip_prefix("data: ") {
                        data_line = rest;
                    } else if let Some(rest) = line.strip_prefix("data:") {
                        data_line = rest.trim_start();
                    }
                }
                if data_line.is_empty() || data_line == "[DONE]" {
                    continue;
                }
                match parse_anthropic_event(data_line) {
                    Ok(model_events) => {
                        for event in model_events {
                            events.push(Ok(event));
                        }
                    }
                    Err(e) => {
                        events.push(Err(e));
                    }
                }
            }
        } else if trimmed.starts_with('{') {
            // Non-streaming JSON response - parse directly
            match parse_anthropic_json_response(trimmed) {
                Ok(model_events) => {
                    for event in model_events {
                        events.push(Ok(event));
                    }
                }
                Err(e) => {
                    events.push(Err(e));
                }
            }
        } else {
            events.push(Err(ModelError::StreamParse(format!(
                "unexpected response format: {}...",
                &trimmed[..trimmed.len().min(50)]
            ))));
        }

        Ok(Box::pin(futures::stream::iter(events)))
    }
}

fn parse_anthropic_event(data: &str) -> Result<Vec<ModelEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(data).map_err(|e| ModelError::StreamParse(e.to_string()))?;

    let event_type = value["type"].as_str().unwrap_or("");
    let mut events = Vec::new();

    match event_type {
        "content_block_delta" => {
            if let Some(text) = value["delta"]["text"].as_str() {
                if !text.is_empty() {
                    events.push(ModelEvent::TokenDelta(text.to_string()));
                }
            }
        }
        "message_delta" => {
            if let Some(stop_reason) = value["delta"]["stop_reason"].as_str() {
                if stop_reason == "end_turn"
                    || stop_reason == "max_tokens"
                    || stop_reason == "stop_sequence"
                    || stop_reason == "tool_use"
                {
                    let usage_value = &value["usage"];
                    let usage = if usage_value.is_object() {
                        Some(crate::ModelUsage {
                            input_tokens: usage_value["input_tokens"].as_u64().unwrap_or(0),
                            output_tokens: usage_value["output_tokens"].as_u64().unwrap_or(0),
                        })
                    } else {
                        None
                    };
                    events.push(ModelEvent::Completed { usage });
                }
            }
        }
        "message_start" | "content_block_start" | "content_block_stop" | "ping" => {
            // No model events to emit for these
        }
        "error" => {
            let msg = value["error"]["message"]
                .as_str()
                .unwrap_or("Unknown Anthropic API error");
            events.push(ModelEvent::Failed {
                message: msg.to_string(),
            });
        }
        _ => {
            // Unknown event type — skip
        }
    }

    Ok(events)
}

/// Parse a non-streaming (JSON) response from the Anthropic Messages API.
/// The proxy may return a complete JSON object instead of SSE events when
/// `stream: true` is requested but the proxy does not support streaming.
fn parse_anthropic_json_response(data: &str) -> Result<Vec<ModelEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(data).map_err(|e| ModelError::StreamParse(e.to_string()))?;

    let mut events = Vec::new();

    // Extract text content from the response
    if let Some(content) = value["content"].as_array() {
        for block in content {
            if block["type"].as_str() == Some("text") {
                if let Some(text) = block["text"].as_str() {
                    if !text.is_empty() {
                        events.push(ModelEvent::TokenDelta(text.to_string()));
                    }
                }
            }
        }
    }

    // Check for completion
    let stop_reason = value["stop_reason"].as_str().unwrap_or("");
    if stop_reason == "end_turn"
        || stop_reason == "max_tokens"
        || stop_reason == "stop_sequence"
        || stop_reason == "tool_use"
    {
        let usage_value = &value["usage"];
        let usage = if usage_value.is_object() {
            Some(crate::ModelUsage {
                input_tokens: usage_value["input_tokens"].as_u64().unwrap_or(0),
                output_tokens: usage_value["output_tokens"].as_u64().unwrap_or(0),
            })
        } else {
            None
        };
        events.push(ModelEvent::Completed { usage });
    }

    // Check for error — handle both {"type":"error"} (standard) and {"error":{}} (proxy) formats
    if value["type"].as_str() == Some("error") || value["error"].is_object() {
        let msg = value["error"]["message"]
            .as_str()
            .or_else(|| value["error"]["type"].as_str())
            .unwrap_or("Unknown Anthropic API error");
        events.push(ModelEvent::Failed {
            message: msg.to_string(),
        });
    }

    Ok(events)
}

#[async_trait]
impl crate::ModelClient for AnthropicClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        self.send_streaming(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModelClient;

    #[test]
    fn builds_anthropic_messages_request() {
        let config = AnthropicConfig {
            base_url: "https://api.anthropic.com".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            default_model: "claude-sonnet-4-20250514".into(),
            max_tokens: 4096,
            headers: Vec::new(),
            capability_overrides: None,
        };
        let client = AnthropicClient::new(config);
        let request = ModelRequest::user_text("fast", "hello")
            .with_system_prompt("You are helpful.")
            .add_message("assistant", "hi there");

        let body = client.build_messages_request(&request);

        // System prompt should be top-level, not in messages
        assert_eq!(body["system"], "You are helpful.");
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(body["model"], "claude-sonnet-4-20250514");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], 4096);
    }

    #[test]
    fn builds_request_with_tools() {
        let config = AnthropicConfig::default();
        let client = AnthropicClient::new(config);
        let request = ModelRequest::user_text("fast", "read README")
            .with_tools(vec![crate::ToolDefinition {
                name: "fs.read".into(),
                description: "Read a file".into(),
                parameters: serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            }]);

        let body = client.build_messages_request(&request);
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools[0]["name"], "fs_read");
        assert!(tools[0]["input_schema"].is_object());
    }

    #[test]
    fn parses_content_block_delta() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let events = parse_anthropic_event(data).unwrap();
        assert_eq!(events, vec![ModelEvent::TokenDelta("Hello".into())]);
    }

    #[test]
    fn parses_message_delta_end_turn() {
        let data = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"input_tokens":10,"output_tokens":5}}"#;
        let events = parse_anthropic_event(data).unwrap();
        assert!(
            matches!(&events[0], ModelEvent::Completed { usage: Some(u) } if u.input_tokens == 10 && u.output_tokens == 5)
        );
    }

    #[test]
    fn parses_error_event() {
        let data = r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;
        let events = parse_anthropic_event(data).unwrap();
        assert!(matches!(&events[0], ModelEvent::Failed { message } if message == "Overloaded"));
    }

    #[test]
    fn ignores_ping_and_start_events() {
        let data = r#"{"type":"ping"}"#;
        let events = parse_anthropic_event(data).unwrap();
        assert!(events.is_empty());

        let data = r#"{"type":"message_start","message":{"id":"msg_123"}}"#;
        let events = parse_anthropic_event(data).unwrap();
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn streams_from_wiremock_server() {
        use futures::StreamExt;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let sse_body = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" there\"}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":5,\"output_tokens\":3}}\n\n";

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse_body))
            .mount(&mock_server)
            .await;

        let config = AnthropicConfig {
            base_url: mock_server.uri(),
            api_key_env: "KAIROX_ANTHROPIC_KEY".into(),
            default_model: "test-model".into(),
            max_tokens: 4096,
            headers: Vec::new(),
            capability_overrides: None,
        };

        std::env::set_var("KAIROX_ANTHROPIC_KEY", "test-key");
        let client = AnthropicClient::new(config);
        let stream: BoxStream<'static, Result<ModelEvent>> = client
            .stream(ModelRequest::user_text("fast", "hello"))
            .await
            .unwrap();

        let events: Vec<Result<ModelEvent>> = stream.collect().await;

        assert!(events
            .iter()
            .any(|e| matches!(e, Ok(ModelEvent::TokenDelta(t)) if t == "Hi")));
        assert!(events
            .iter()
            .any(|e| matches!(e, Ok(ModelEvent::TokenDelta(t)) if t == " there")));
        assert!(events
            .iter()
            .any(|e| matches!(e, Ok(ModelEvent::Completed { .. }))));
        std::env::remove_var("KAIROX_ANTHROPIC_KEY");
    }
}

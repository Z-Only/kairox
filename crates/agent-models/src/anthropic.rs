//! Anthropic Messages API client.
//!
//! Supports the `/v1/messages` endpoint with SSE streaming,
//! authenticating via the `x-api-key` header.

use crate::{ModelError, ModelEvent, ModelRequest, Result};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

        // Build a reverse name map so we can map Anthropic-safe tool names
        // (e.g. "shell_exec") back to the original names (e.g. "shell.exec")
        // when the model requests a tool_use.
        let name_map: HashMap<String, String> = request
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
                (safe_name, t.name.clone())
            })
            .collect();

        // Collect raw SSE events into a type-erased boxed stream.
        let raw_stream: BoxStream<'static, Result<AnthropicRawEvent>> = response
            .bytes_stream()
            .eventsource()
            .map_err(|e| ModelError::StreamParse(e.to_string()))
            .and_then(|event| async move {
                if event.data == "[DONE]" {
                    Ok(None)
                } else {
                    parse_anthropic_raw_events(&event.data).map(Some)
                }
            })
            .filter_map(
                |result: std::result::Result<Option<Vec<AnthropicRawEvent>>, ModelError>| async move {
                    match result {
                        Ok(Some(raw_events)) => {
                            Some(futures::stream::iter(raw_events.into_iter().map(Ok)).boxed())
                        }
                        Ok(None) => Some(futures::stream::empty::<Result<AnthropicRawEvent>>().boxed()),
                        Err(e) => Some(futures::stream::once(async { Err(e) }).boxed()),
                    }
                },
            )
            .flatten()
            .boxed();

        // Process raw events through the accumulator and flush any pending
        // tool calls when the stream ends. Anthropic sends tool_use blocks
        // as content_block_start → content_block_delta (input_json_delta) →
        // content_block_stop, so we accumulate arguments across chunks and
        // emit ToolCallRequested when content_block_stop fires.
        let stream: BoxStream<'static, Result<ModelEvent>> = {
            let mut acc = AnthropicToolCallAccumulator::new(name_map);
            Box::pin(async_stream::stream! {
                let mut raw_stream = raw_stream;

                while let Some(item) = raw_stream.next().await {
                    let events: Vec<Result<ModelEvent>> = match item {
                        Ok(raw) => acc.process(raw).into_iter().map(Ok).collect(),
                        Err(e) => vec![Err(e)],
                    };
                    for event in events {
                        yield event;
                    }
                }

                // Flush any pending tool calls that haven't been emitted yet.
                // This handles edge cases where content_block_stop might be
                // missed or the stream ends unexpectedly.
                for event in acc.flush() {
                    yield Ok(event);
                }
            })
        };

        Ok(stream)
    }
}

// ---------------------------------------------------------------------------
// Streaming tool call accumulation
// ---------------------------------------------------------------------------

/// Internal events during Anthropic SSE stream processing.
/// Tool call arguments arrive across multiple `content_block_delta` chunks
/// and must be accumulated before emitting a `ModelEvent::ToolCallRequested`.
enum AnthropicRawEvent {
    /// A regular model event (text delta, completion, error — no accumulation needed).
    Event(ModelEvent),
    /// A `content_block_start` with `type: "tool_use"` — begins a new tool call.
    ToolUseStarted { id: String, name: String },
    /// A `content_block_delta` with `type: "input_json_delta"` — partial JSON arguments.
    ToolUseArgumentDelta { partial_json: String },
    /// A `content_block_stop` — signals the current content block is complete.
    ToolUseFinished,
}

/// Accumulates tool call arguments across SSE chunks.
///
/// Anthropic sends tool calls as:
/// 1. `content_block_start` → { id, name }
/// 2. One or more `content_block_delta` → { partial_json } fragments
/// 3. `content_block_stop`
///
/// Only after step 3 do we have the complete arguments JSON and can emit
/// a `ModelEvent::ToolCallRequested`.
struct AnthropicToolCallAccumulator {
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
    fn new(name_map: HashMap<String, String>) -> Self {
        Self {
            pending: None,
            name_map,
        }
    }

    /// Flush any remaining pending tool calls into model events.
    /// Called when the stream ends to emit any tool calls that haven't
    /// been finalized by a content_block_stop event.
    fn flush(&mut self) -> Vec<ModelEvent> {
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
    fn process(&mut self, raw: AnthropicRawEvent) -> Vec<ModelEvent> {
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

/// Parse a single SSE event from the Anthropic Messages API into a list of
/// raw events. Text deltas and completion events are emitted immediately;
/// tool_use blocks are split into start/delta/finished events that will be
/// accumulated by `AnthropicToolCallAccumulator`.
fn parse_anthropic_raw_events(data: &str) -> Result<Vec<AnthropicRawEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(data).map_err(|e| ModelError::StreamParse(e.to_string()))?;

    let event_type = value["type"].as_str().unwrap_or("");
    let mut events = Vec::new();

    match event_type {
        "content_block_start" => {
            let block_type = value["content_block"]["type"].as_str().unwrap_or("");
            if block_type == "tool_use" {
                let id = value["content_block"]["id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let name = value["content_block"]["name"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                if !id.is_empty() && !name.is_empty() {
                    events.push(AnthropicRawEvent::ToolUseStarted { id, name });
                }
            }
        }
        "content_block_delta" => {
            let delta_type = value["delta"]["type"].as_str().unwrap_or("");
            match delta_type {
                "text_delta" => {
                    if let Some(text) = value["delta"]["text"].as_str() {
                        if !text.is_empty() {
                            events.push(AnthropicRawEvent::Event(ModelEvent::TokenDelta(
                                text.to_string(),
                            )));
                        }
                    }
                }
                "input_json_delta" => {
                    if let Some(partial) = value["delta"]["partial_json"].as_str() {
                        events.push(AnthropicRawEvent::ToolUseArgumentDelta {
                            partial_json: partial.to_string(),
                        });
                    }
                }
                _ => {}
            }
        }
        "content_block_stop" => {
            // This fires for every content block (text or tool_use).
            // The accumulator will only act on it if there's a pending tool call.
            events.push(AnthropicRawEvent::ToolUseFinished);
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
                    events.push(AnthropicRawEvent::Event(ModelEvent::Completed { usage }));
                }
            }
        }
        "message_start" | "ping" => {
            // No model events to emit for these
        }
        "error" => {
            let msg = value["error"]["message"]
                .as_str()
                .unwrap_or("Unknown Anthropic API error");
            events.push(AnthropicRawEvent::Event(ModelEvent::Failed {
                message: msg.to_string(),
            }));
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
#[allow(dead_code)]
fn parse_anthropic_json_response(data: &str) -> Result<Vec<ModelEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(data).map_err(|e| ModelError::StreamParse(e.to_string()))?;

    let mut events = Vec::new();

    // Extract content from the response (both text and tool_use blocks)
    if let Some(content) = value["content"].as_array() {
        for block in content {
            match block["type"].as_str().unwrap_or("") {
                "text" => {
                    if let Some(text) = block["text"].as_str() {
                        if !text.is_empty() {
                            events.push(ModelEvent::TokenDelta(text.to_string()));
                        }
                    }
                }
                "tool_use" => {
                    let id = block["id"].as_str().unwrap_or("").to_string();
                    let name = block["name"].as_str().unwrap_or("").to_string();
                    let arguments = block["input"].clone();
                    if !id.is_empty() && !name.is_empty() {
                        events.push(ModelEvent::ToolCallRequested {
                            tool_call_id: id,
                            tool_id: name,
                            arguments,
                        });
                    }
                }
                _ => {}
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
    fn builds_anthropic_request_with_tool_use_and_result() {
        let config = AnthropicConfig::default();
        let client = AnthropicClient::new(config);

        // Simulate a conversation where:
        // 1. User asks "list files"
        // 2. Assistant responds with tool_use (shell.exec)
        // 3. Tool result is provided
        let request = ModelRequest::user_text("fast", "list files")
            .with_tools(vec![crate::ToolDefinition {
                name: "shell.exec".into(),
                description: "Execute a shell command".into(),
                parameters: serde_json::json!({"type": "object"}),
            }])
            .add_assistant_with_tools(
                "I'll list the files.",
                vec![crate::ToolCall {
                    id: "toolu_01".into(),
                    name: "shell.exec".into(),
                    arguments: serde_json::json!({"command": "ls"}),
                }],
            )
            .add_tool_result(
                "toolu_01",
                "file1.txt
file2.rs",
            );

        let body = client.build_messages_request(&request);
        let messages = body["messages"].as_array().unwrap();

        // Message 0: user "list files"
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"], "list files");

        // Message 1: assistant with text + tool_use content block
        assert_eq!(messages[1]["role"], "assistant");
        let content_blocks = messages[1]["content"].as_array().unwrap();
        assert_eq!(content_blocks.len(), 2);
        // Text block
        assert_eq!(content_blocks[0]["type"], "text");
        assert_eq!(content_blocks[0]["text"], "I'll list the files.");
        // Tool use block
        assert_eq!(content_blocks[1]["type"], "tool_use");
        assert_eq!(content_blocks[1]["id"], "toolu_01");
        assert_eq!(content_blocks[1]["name"], "shell_exec"); // name mapped to Anthropic-safe
        assert_eq!(content_blocks[1]["input"]["command"], "ls");

        // Message 2: user with tool_result content block
        assert_eq!(messages[2]["role"], "user");
        let result_blocks = messages[2]["content"].as_array().unwrap();
        assert_eq!(result_blocks.len(), 1);
        assert_eq!(result_blocks[0]["type"], "tool_result");
        assert_eq!(result_blocks[0]["tool_use_id"], "toolu_01");
        assert_eq!(result_blocks[0]["content"], "file1.txt\nfile2.rs");
    }

    #[test]
    fn builds_anthropic_request_with_empty_assistant_text_and_tool_calls() {
        let config = AnthropicConfig::default();
        let client = AnthropicClient::new(config);

        // When the model responds with only tool calls (no text), the assistant
        // message should still be included with tool_use blocks.
        let request = ModelRequest::user_text("fast", "read file")
            .with_tools(vec![crate::ToolDefinition {
                name: "fs.read".into(),
                description: "Read a file".into(),
                parameters: serde_json::json!({"type": "object"}),
            }])
            .add_assistant_with_tools(
                "", // empty text
                vec![crate::ToolCall {
                    id: "toolu_02".into(),
                    name: "fs.read".into(),
                    arguments: serde_json::json!({"path": "README.md"}),
                }],
            )
            .add_tool_result("toolu_02", "# My Project");

        let body = client.build_messages_request(&request);
        let messages = body["messages"].as_array().unwrap();

        // Assistant message should be present with empty text and tool_use block
        assert_eq!(messages[1]["role"], "assistant");
        let content_blocks = messages[1]["content"].as_array().unwrap();
        // Only tool_use block (no text block since content is empty)
        assert_eq!(content_blocks.len(), 1);
        assert_eq!(content_blocks[0]["type"], "tool_use");
        assert_eq!(content_blocks[0]["id"], "toolu_02");

        // Tool result follows as user message
        assert_eq!(messages[2]["role"], "user");
        let result_blocks = messages[2]["content"].as_array().unwrap();
        assert_eq!(result_blocks[0]["type"], "tool_result");
        assert_eq!(result_blocks[0]["tool_use_id"], "toolu_02");
    }

    #[test]
    fn parses_content_block_delta_text() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let events = parse_anthropic_raw_events(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AnthropicRawEvent::Event(ModelEvent::TokenDelta(t)) => assert_eq!(t, "Hello"),
            _ => panic!("expected TokenDelta event"),
        }
    }

    #[test]
    fn parses_content_block_start_tool_use() {
        let data = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_01","name":"shell_exec"}}"#;
        let events = parse_anthropic_raw_events(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AnthropicRawEvent::ToolUseStarted { id, name } => {
                assert_eq!(id, "toolu_01");
                assert_eq!(name, "shell_exec");
            }
            _ => panic!("expected ToolUseStarted event"),
        }
    }

    #[test]
    fn parses_content_block_delta_input_json() {
        let data = r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"command\":"}}"#;
        let events = parse_anthropic_raw_events(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AnthropicRawEvent::ToolUseArgumentDelta { partial_json } => {
                assert_eq!(partial_json, "{\"command\":");
            }
            _ => panic!("expected ToolUseArgumentDelta event"),
        }
    }

    #[test]
    fn parses_content_block_stop() {
        let data = r#"{"type":"content_block_stop","index":1}"#;
        let events = parse_anthropic_raw_events(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AnthropicRawEvent::ToolUseFinished => {}
            _ => panic!("expected ToolUseFinished event"),
        }
    }

    #[test]
    fn parses_message_delta_end_turn() {
        let data = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"input_tokens":10,"output_tokens":5}}"#;
        let events = parse_anthropic_raw_events(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AnthropicRawEvent::Event(ModelEvent::Completed { usage: Some(u) }) => {
                assert_eq!(u.input_tokens, 10);
                assert_eq!(u.output_tokens, 5);
            }
            _ => panic!("expected Completed event"),
        }
    }

    #[test]
    fn parses_message_delta_tool_use_stop() {
        let data = r#"{"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"input_tokens":20,"output_tokens":10}}"#;
        let events = parse_anthropic_raw_events(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AnthropicRawEvent::Event(ModelEvent::Completed { usage }) => {
                assert_eq!(usage.as_ref().unwrap().output_tokens, 10);
            }
            _ => panic!("expected Completed event"),
        }
    }

    #[test]
    fn parses_error_event() {
        let data = r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;
        let events = parse_anthropic_raw_events(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AnthropicRawEvent::Event(ModelEvent::Failed { message }) => {
                assert_eq!(message, "Overloaded");
            }
            _ => panic!("expected Failed event"),
        }
    }

    #[test]
    fn ignores_ping_and_start_events() {
        let data = r#"{"type":"ping"}"#;
        let events = parse_anthropic_raw_events(data).unwrap();
        assert!(events.is_empty());

        let data = r#"{"type":"message_start","message":{"id":"msg_123"}}"#;
        let events = parse_anthropic_raw_events(data).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn accumulator_accumulates_tool_call_across_chunks() {
        let name_map = HashMap::from([
            ("shell_exec".to_string(), "shell.exec".to_string()),
            ("fs_read".to_string(), "fs.read".to_string()),
        ]);
        let mut acc = AnthropicToolCallAccumulator::new(name_map);

        // Text delta passes through immediately
        let events = acc.process(AnthropicRawEvent::Event(ModelEvent::TokenDelta(
            "I'll list files.".into(),
        )));
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ModelEvent::TokenDelta(t) if t == "I'll list files."));

        // Tool use starts — no output yet
        let events = acc.process(AnthropicRawEvent::ToolUseStarted {
            id: "toolu_01".into(),
            name: "shell_exec".into(),
        });
        assert!(events.is_empty());

        // Argument fragments — still accumulating
        let events = acc.process(AnthropicRawEvent::ToolUseArgumentDelta {
            partial_json: "{\"command\":".into(),
        });
        assert!(events.is_empty());
        let events = acc.process(AnthropicRawEvent::ToolUseArgumentDelta {
            partial_json: " \"ls\"}".into(),
        });
        assert!(events.is_empty());

        // Content block stop — flush the completed tool call
        let events = acc.process(AnthropicRawEvent::ToolUseFinished);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => {
                assert_eq!(tool_call_id, "toolu_01");
                assert_eq!(tool_id, "shell.exec"); // name mapped back from shell_exec
                assert_eq!(arguments["command"], "ls");
            }
            _ => panic!("expected ToolCallRequested"),
        }

        // Text block stop — no pending tool call, so nothing to emit
        let events = acc.process(AnthropicRawEvent::ToolUseFinished);
        assert!(events.is_empty());
    }

    #[test]
    fn accumulator_handles_unknown_tool_name() {
        let name_map = HashMap::new(); // empty map
        let mut acc = AnthropicToolCallAccumulator::new(name_map);

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
                // Unknown name stays as-is (no mapping available)
                assert_eq!(tool_id, "custom_tool");
            }
            _ => panic!("expected ToolCallRequested"),
        }
    }

    #[test]
    fn parse_anthropic_json_response_handles_tool_use() {
        let data = r#"{
            "id": "msg_01",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "I'll list the files."},
                {"type": "tool_use", "id": "toolu_01", "name": "shell_exec", "input": {"command": "ls"}}
            ],
            "model": "claude-sonnet-4-20250514",
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 100, "output_tokens": 50}
        }"#;
        let events = parse_anthropic_json_response(data).unwrap();
        assert_eq!(events.len(), 3); // TokenDelta + ToolCallRequested + Completed
        assert!(matches!(&events[0], ModelEvent::TokenDelta(t) if t == "I'll list the files."));
        match &events[1] {
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => {
                assert_eq!(tool_call_id, "toolu_01");
                assert_eq!(tool_id, "shell_exec");
                assert_eq!(arguments["command"], "ls");
            }
            _ => panic!("expected ToolCallRequested"),
        }
        assert!(
            matches!(&events[2], ModelEvent::Completed { usage: Some(u) } if u.input_tokens == 100 && u.output_tokens == 50)
        );
    }

    #[tokio::test]
    async fn streams_from_wiremock_server() {
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

    #[tokio::test]
    async fn streams_tool_use_from_wiremock_server() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Simulate an Anthropic SSE stream where the model calls a tool
        let sse_body = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"I'll list the files.\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"shell_exec\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\":\\\"ls\\\"}\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":1}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"input_tokens\":50,\"output_tokens\":30}}\n\n";

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse_body))
            .mount(&mock_server)
            .await;

        let config = AnthropicConfig {
            base_url: mock_server.uri(),
            api_key_env: "KAIROX_ANTHROPIC_WIREMOCK_KEY".into(),
            default_model: "test-model".into(),
            max_tokens: 4096,
            headers: Vec::new(),
            capability_overrides: None,
        };

        std::env::set_var("KAIROX_ANTHROPIC_WIREMOCK_KEY", "test-key");
        let client = AnthropicClient::new(config);

        // Provide tools so the name_map maps "shell_exec" → "shell.exec"
        let request = ModelRequest::user_text("claude", "list files").with_tools(vec![
            crate::ToolDefinition {
                name: "shell.exec".into(),
                description: "Execute shell commands".into(),
                parameters: serde_json::json!({"type": "object"}),
            },
        ]);

        let stream: BoxStream<'static, Result<ModelEvent>> = client.stream(request).await.unwrap();

        let events: Vec<Result<ModelEvent>> = stream.collect().await;

        // Should have: TokenDelta("I'll list the files."),
        // ToolCallRequested("toolu_01", "shell.exec", {command: "ls"}),
        // Completed
        let model_events: Vec<ModelEvent> = events.into_iter().filter_map(|e| e.ok()).collect();

        let text_deltas: Vec<&String> = model_events
            .iter()
            .filter_map(|e| match e {
                ModelEvent::TokenDelta(t) => Some(t),
                _ => None,
            })
            .collect();
        assert!(text_deltas.iter().any(|t| t.contains("list the files")));

        let tool_calls: Vec<&ModelEvent> = model_events
            .iter()
            .filter(|e| matches!(e, ModelEvent::ToolCallRequested { .. }))
            .collect();
        assert_eq!(
            tool_calls.len(),
            1,
            "expected exactly one ToolCallRequested, got: {model_events:?}"
        );
        match tool_calls[0] {
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => {
                assert_eq!(tool_call_id, "toolu_01");
                assert_eq!(tool_id, "shell.exec"); // mapped back from shell_exec
                assert_eq!(arguments["command"], "ls");
            }
            _ => unreachable!(),
        }

        assert!(model_events
            .iter()
            .any(|e| matches!(e, ModelEvent::Completed { .. })));

        std::env::remove_var("KAIROX_ANTHROPIC_WIREMOCK_KEY");
    }

    #[tokio::test]
    async fn streams_multi_chunk_tool_arguments() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Simulate tool call with arguments split across multiple chunks
        let sse_body = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_2\"}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_02\",\"name\":\"fs_read\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"README\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\".md\\\"}\"}}\n\nevent: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"input_tokens\":10,\"output_tokens\":8}}\n\n";

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse_body))
            .mount(&mock_server)
            .await;

        let config = AnthropicConfig {
            base_url: mock_server.uri(),
            api_key_env: "KAIROX_ANTHROPIC_MULTI_KEY".into(),
            default_model: "test-model".into(),
            max_tokens: 4096,
            headers: Vec::new(),
            capability_overrides: None,
        };

        std::env::set_var("KAIROX_ANTHROPIC_MULTI_KEY", "test-key");
        let client = AnthropicClient::new(config);

        let request = ModelRequest::user_text("claude", "read readme").with_tools(vec![
            crate::ToolDefinition {
                name: "fs.read".into(),
                description: "Read a file".into(),
                parameters: serde_json::json!({"type": "object"}),
            },
        ]);

        let stream: BoxStream<'static, Result<ModelEvent>> = client.stream(request).await.unwrap();
        let events: Vec<ModelEvent> = stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();

        let tool_call = events
            .iter()
            .find(|e| matches!(e, ModelEvent::ToolCallRequested { .. }));
        assert!(
            tool_call.is_some(),
            "expected ToolCallRequested in: {events:?}"
        );
        match tool_call.unwrap() {
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => {
                assert_eq!(tool_call_id, "toolu_02");
                assert_eq!(tool_id, "fs.read"); // mapped back from fs_read
                assert_eq!(arguments["path"], "README.md");
            }
            _ => unreachable!(),
        }

        std::env::remove_var("KAIROX_ANTHROPIC_MULTI_KEY");
    }
}

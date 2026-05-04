use crate::{ModelError, ModelEvent, ModelRequest, Result};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAiCompatibleConfig {
    pub base_url: String,
    pub api_key_env: String,
    pub default_model: String,
    pub headers: Vec<(String, String)>,
    pub capability_overrides: Option<crate::ModelCapabilities>,
}

impl OpenAiCompatibleConfig {
    pub fn default_capabilities(&self) -> crate::ModelCapabilities {
        self.capability_overrides
            .clone()
            .unwrap_or(crate::ModelCapabilities {
                streaming: true,
                tool_calling: true,
                json_schema: true,
                vision: false,
                reasoning_controls: false,
                context_window: 128_000,
                output_limit: 16_384,
                local_model: false,
            })
    }

    fn api_key(&self) -> Option<String> {
        std::env::var(&self.api_key_env).ok()
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleClient {
    config: OpenAiCompatibleConfig,
    http: Client,
}

impl OpenAiCompatibleClient {
    pub fn new(config: OpenAiCompatibleConfig) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build reqwest client");
        Self { config, http }
    }

    pub fn from_config(config: OpenAiCompatibleConfig, http: Client) -> Self {
        Self { config, http }
    }

    fn build_chat_request(&self, request: &ModelRequest) -> Result<serde_json::Value> {
        let mut messages = Vec::new();

        if let Some(ref system_prompt) = request.system_prompt {
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
                    "content": if msg.content.is_empty() { serde_json::Value::Null } else { serde_json::json!(msg.content) },
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

        Ok(body)
    }

    async fn send_streaming(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        let body = self.build_chat_request(&request)?;
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );

        let mut builder = self.http.post(&url).json(&body);
        if let Some(api_key) = self.config.api_key() {
            builder = builder.bearer_auth(&api_key);
        }
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

        // Collect raw SSE events into a type-erased boxed stream.
        let raw_stream: BoxStream<'static, Result<OpenAiChunkEvent>> = response
            .bytes_stream()
            .eventsource()
            .map_err(|e| ModelError::StreamParse(e.to_string()))
            .and_then(|event| async move {
                if event.data == "[DONE]" {
                    Ok(None)
                } else {
                    parse_openai_chunk(&event.data).map(Some)
                }
            })
            .filter_map(
                |result: std::result::Result<Option<Vec<OpenAiChunkEvent>>, ModelError>| async move {
                    match result {
                        Ok(Some(events)) => {
                            Some(futures::stream::iter(events.into_iter().map(Ok)).boxed())
                        }
                        Ok(None) => Some(futures::stream::empty::<Result<OpenAiChunkEvent>>().boxed()),
                        Err(e) => Some(futures::stream::once(async { Err(e) }).boxed()),
                    }
                },
            )
            .flatten()
            .boxed();

        // Use async_stream to process raw events through the accumulator and
        // flush any pending tool calls when the input stream ends.
        let stream: BoxStream<'static, Result<ModelEvent>> = {
            let mut acc = OpenAiToolCallAccumulator::new();
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
                // This is critical: OpenAI finishes tool_calls with finish_reason
                // but doesn't signal "end of arguments" per tool — we only know
                // the arguments are complete when the stream ends.
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

/// Internal events during OpenAI SSE stream processing.
/// Tool call arguments arrive across multiple chunks and must be accumulated
/// before emitting a `ModelEvent::ToolCallRequested`.
enum OpenAiChunkEvent {
    /// A regular model event (text delta, completion — no accumulation needed).
    Event(ModelEvent),
    /// A tool call start chunk (has non-empty id + name, may also have partial arguments).
    ToolCallStarted {
        index: usize,
        id: String,
        name: String,
    },
    /// A tool call argument delta chunk (continuation of arguments).
    ToolCallArgumentDelta {
        index: usize,
        partial_arguments: String,
    },
}

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
struct OpenAiToolCallAccumulator {
    /// Tool calls being accumulated, keyed by their index in the `tool_calls` array.
    pending: HashMap<usize, PendingOpenAiToolCall>,
}

struct PendingOpenAiToolCall {
    id: String,
    name: String,
    arguments_buffer: String,
}

impl OpenAiToolCallAccumulator {
    fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Process a raw chunk event and return zero or more completed model events.
    fn process(&mut self, raw: OpenAiChunkEvent) -> Vec<ModelEvent> {
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
    fn flush(&mut self) -> Vec<ModelEvent> {
        let pending = std::mem::take(&mut self.pending);
        pending
            .into_values()
            .map(|p| self.finalize_pending(p))
            .collect()
    }
}

fn parse_openai_chunk(data: &str) -> Result<Vec<OpenAiChunkEvent>> {
    let chunk: serde_json::Value =
        serde_json::from_str(data).map_err(|e| ModelError::StreamParse(e.to_string()))?;

    let mut events = Vec::new();

    if let Some(choices) = chunk["choices"].as_array() {
        for choice in choices {
            let delta = &choice["delta"];

            // Content token delta
            if let Some(content) = delta["content"].as_str() {
                if !content.is_empty() {
                    events.push(OpenAiChunkEvent::Event(ModelEvent::TokenDelta(
                        content.to_string(),
                    )));
                }
            }

            // Tool calls — split into start and argument-delta events
            if let Some(tool_calls) = delta["tool_calls"].as_array() {
                for tc in tool_calls {
                    let index = tc["index"].as_u64().unwrap_or(0) as usize;
                    let id = tc["id"].as_str().unwrap_or("").to_string();
                    let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                    let arguments_str = tc["function"]["arguments"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    if !id.is_empty() && !name.is_empty() {
                        // First chunk for this tool call — has id and name
                        events.push(OpenAiChunkEvent::ToolCallStarted { index, id, name });
                        // If arguments are also present in this chunk, emit a delta
                        if !arguments_str.is_empty() {
                            events.push(OpenAiChunkEvent::ToolCallArgumentDelta {
                                index,
                                partial_arguments: arguments_str,
                            });
                        }
                    } else if !arguments_str.is_empty() {
                        // Continuation chunk — only argument delta
                        events.push(OpenAiChunkEvent::ToolCallArgumentDelta {
                            index,
                            partial_arguments: arguments_str,
                        });
                    }
                }
            }

            // Finish reason
            if let Some(finish) = choice["finish_reason"].as_str() {
                if finish == "stop" || finish == "tool_calls" {
                    let usage_value = &chunk["usage"];
                    let usage = if usage_value.is_object() {
                        Some(crate::ModelUsage {
                            input_tokens: usage_value["prompt_tokens"].as_u64().unwrap_or(0),
                            output_tokens: usage_value["completion_tokens"].as_u64().unwrap_or(0),
                        })
                    } else {
                        None
                    };
                    events.push(OpenAiChunkEvent::Event(ModelEvent::Completed { usage }));
                }
            }
        }
    }

    Ok(events)
}

#[async_trait]
impl crate::ModelClient for OpenAiCompatibleClient {
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
    fn builds_chat_request_with_system_prompt_and_messages() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.openai.com/v1".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            default_model: "gpt-4.1".into(),
            headers: vec![],
            capability_overrides: None,
        };
        let client = OpenAiCompatibleClient::new(config);
        let request = ModelRequest::user_text("fast", "hello")
            .with_system_prompt("You are helpful.")
            .add_message("assistant", "hi there");

        let body = client.build_chat_request(&request).unwrap();

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "You are helpful.");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(body["model"], "gpt-4.1");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn builds_chat_request_with_tools() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.openai.com/v1".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            default_model: "gpt-4.1".into(),
            headers: vec![],
            capability_overrides: None,
        };
        let client = OpenAiCompatibleClient::new(config);
        let request = ModelRequest::user_text("fast", "read README")
            .with_tools(vec![crate::ToolDefinition {
                name: "fs.read".into(),
                description: "Read a file".into(),
                parameters: serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            }]);

        let body = client.build_chat_request(&request).unwrap();
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "fs.read");
    }

    #[test]
    fn builds_chat_request_with_tool_calls_and_results() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.openai.com/v1".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            default_model: "gpt-4.1".into(),
            headers: vec![],
            capability_overrides: None,
        };
        let client = OpenAiCompatibleClient::new(config);

        // Simulate a conversation with tool calls and results
        let request = ModelRequest::user_text("fast", "list files")
            .with_tools(vec![crate::ToolDefinition {
                name: "shell.exec".into(),
                description: "Execute a shell command".into(),
                parameters: serde_json::json!({"type": "object"}),
            }])
            .add_assistant_with_tools(
                "I'll list the files.",
                vec![crate::ToolCall {
                    id: "call_abc".into(),
                    name: "shell.exec".into(),
                    arguments: serde_json::json!({"command": "ls"}),
                }],
            )
            .add_tool_result(
                "call_abc",
                "file1.txt
file2.rs",
            );

        let body = client.build_chat_request(&request).unwrap();
        let messages = body["messages"].as_array().unwrap();

        // Message 0: user "list files"
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"], "list files");

        // Message 1: assistant with tool_calls array
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["content"], "I'll list the files.");
        let tool_calls = messages[1]["tool_calls"].as_array().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0]["id"], "call_abc");
        assert_eq!(tool_calls[0]["type"], "function");
        assert_eq!(tool_calls[0]["function"]["name"], "shell.exec");

        // Message 2: tool result with tool_call_id
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call_abc");
        assert_eq!(messages[2]["content"], "file1.txt\nfile2.rs");
    }

    #[test]
    fn builds_chat_request_with_empty_assistant_text_and_tool_calls() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.openai.com/v1".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            default_model: "gpt-4.1".into(),
            headers: vec![],
            capability_overrides: None,
        };
        let client = OpenAiCompatibleClient::new(config);

        // When assistant has only tool calls (no text), content should be null
        let request = ModelRequest::user_text("fast", "read file")
            .add_assistant_with_tools(
                "", // empty text
                vec![crate::ToolCall {
                    id: "call_xyz".into(),
                    name: "fs.read".into(),
                    arguments: serde_json::json!({"path": "README.md"}),
                }],
            )
            .add_tool_result("call_xyz", "# My Project");

        let body = client.build_chat_request(&request).unwrap();
        let messages = body["messages"].as_array().unwrap();

        // Messages: [user, assistant, tool]
        // User message
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"], "read file");

        // Assistant message: content is null (empty text), tool_calls present
        assert_eq!(messages[1]["role"], "assistant");
        assert!(messages[1]["content"].is_null());
        let tool_calls = messages[1]["tool_calls"].as_array().unwrap();
        assert_eq!(tool_calls[0]["id"], "call_xyz");
    }

    #[test]
    fn parses_token_delta_from_chunk() {
        let data = r#"{"choices":[{"delta":{"content":"Hello"},"index":0}]}"#;
        let events = parse_openai_chunk(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OpenAiChunkEvent::Event(ModelEvent::TokenDelta(t)) => assert_eq!(t, "Hello"),
            _ => panic!("expected TokenDelta event"),
        }
    }

    #[test]
    fn parses_tool_call_start_from_chunk() {
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"fs.read","arguments":"{\"pa"}}]},"index":0}]}"#;
        let events = parse_openai_chunk(data).unwrap();
        assert_eq!(events.len(), 2); // ToolCallStarted + ToolCallArgumentDelta
        match &events[0] {
            OpenAiChunkEvent::ToolCallStarted { index, id, name } => {
                assert_eq!(*index, 0);
                assert_eq!(id, "call_1");
                assert_eq!(name, "fs.read");
            }
            _ => panic!("expected ToolCallStarted"),
        }
        match &events[1] {
            OpenAiChunkEvent::ToolCallArgumentDelta {
                index,
                partial_arguments,
            } => {
                assert_eq!(*index, 0);
                assert_eq!(partial_arguments, "{\"pa");
            }
            _ => panic!("expected ToolCallArgumentDelta"),
        }
    }

    #[test]
    fn parses_tool_call_argument_delta_chunk() {
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\": \"README.md\"}"}}]},"index":0}]}"#;
        let events = parse_openai_chunk(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OpenAiChunkEvent::ToolCallArgumentDelta {
                index,
                partial_arguments,
            } => {
                assert_eq!(*index, 0);
                assert_eq!(partial_arguments, "th\": \"README.md\"}");
            }
            _ => panic!("expected ToolCallArgumentDelta"),
        }
    }

    #[test]
    fn parses_completion_event() {
        let data = r#"{"choices":[{"finish_reason":"stop","index":0}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#;
        let events = parse_openai_chunk(data).unwrap();
        assert!(matches!(
            &events[0],
            OpenAiChunkEvent::Event(ModelEvent::Completed { usage: Some(u) })
            if u.input_tokens == 10 && u.output_tokens == 5
        ));
    }

    #[test]
    fn accumulator_accumulates_tool_call_across_chunks() {
        let mut acc = OpenAiToolCallAccumulator::new();

        // Text delta passes through immediately
        let events = acc.process(OpenAiChunkEvent::Event(ModelEvent::TokenDelta(
            "Reading file...".into(),
        )));
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ModelEvent::TokenDelta(t) if t == "Reading file..."));

        // Tool call start — no ToolCallRequested yet
        let events = acc.process(OpenAiChunkEvent::ToolCallStarted {
            index: 0,
            id: "call_abc".into(),
            name: "fs.read".into(),
        });
        assert!(events.is_empty());

        // First argument fragment
        let events = acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
            index: 0,
            partial_arguments: "{\"pa".into(),
        });
        assert!(events.is_empty());

        // Second argument fragment
        let events = acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
            index: 0,
            partial_arguments: "th\": \"README.md\"}".into(),
        });
        assert!(events.is_empty());

        // Completion event passes through
        let events = acc.process(OpenAiChunkEvent::Event(ModelEvent::Completed {
            usage: None,
        }));
        assert_eq!(events.len(), 1);

        // Flush remaining pending tool calls
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
    fn accumulator_handles_multiple_tool_calls() {
        let mut acc = OpenAiToolCallAccumulator::new();

        // First tool call starts
        acc.process(OpenAiChunkEvent::ToolCallStarted {
            index: 0,
            id: "call_1".into(),
            name: "fs.read".into(),
        });
        acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
            index: 0,
            partial_arguments: "{\"path\":\"README.md\"}".into(),
        });

        // Second tool call starts
        acc.process(OpenAiChunkEvent::ToolCallStarted {
            index: 1,
            id: "call_2".into(),
            name: "shell.exec".into(),
        });
        acc.process(OpenAiChunkEvent::ToolCallArgumentDelta {
            index: 1,
            partial_arguments: "{\"command\":\"ls\"}".into(),
        });

        // Flush all
        let mut events = acc.flush();
        assert_eq!(events.len(), 2);

        // Order may vary since HashMap doesn't preserve insertion order
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

    #[tokio::test]
    async fn streams_from_wiremock_server() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"},\"index\":0}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\" there\"},\"index\":0}]}\n\ndata: {\"choices\":[{\"finish_reason\":\"stop\",\"index\":0}]}\n\ndata: [DONE]\n\n"
            ))
            .mount(&mock_server)
            .await;

        let config = OpenAiCompatibleConfig {
            base_url: mock_server.uri(),
            api_key_env: "TEST_KEY_NOT_SET".into(),
            default_model: "test-model".into(),
            headers: vec![],
            capability_overrides: None,
        };
        let client = OpenAiCompatibleClient::new(config);
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
    }

    #[tokio::test]
    async fn streams_tool_calls_from_wiremock_server() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Simulate an OpenAI SSE stream where the model calls a tool with
        // arguments split across multiple chunks
        let sse_body = "data: {\"choices\":[{\"delta\":{\"content\":\"Let me list the files.\"},\"index\":0}]}\n\ndata: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_abc\",\"function\":{\"name\":\"shell.exec\",\"arguments\":\"{\\\"command\\\":\\\"ls\\\"}\"}}]},\"index\":0}]}\n\ndata: {\"choices\":[{\"finish_reason\":\"tool_calls\",\"index\":0}]}\n\ndata: [DONE]\n\n";

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse_body))
            .mount(&mock_server)
            .await;

        let config = OpenAiCompatibleConfig {
            base_url: mock_server.uri(),
            api_key_env: "TEST_KEY_OAI_TC".into(),
            default_model: "test-model".into(),
            headers: vec![],
            capability_overrides: None,
        };

        std::env::set_var("TEST_KEY_OAI_TC", "test-key");
        let client = OpenAiCompatibleClient::new(config);
        let stream: BoxStream<'static, Result<ModelEvent>> = client
            .stream(ModelRequest::user_text("fast", "list files"))
            .await
            .unwrap();

        let events: Vec<ModelEvent> = stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();

        assert!(events
            .iter()
            .any(|e| matches!(e, ModelEvent::TokenDelta(t) if t == "Let me list the files.")));

        let tool_calls: Vec<&ModelEvent> = events
            .iter()
            .filter(|e| matches!(e, ModelEvent::ToolCallRequested { .. }))
            .collect();
        assert_eq!(
            tool_calls.len(),
            1,
            "expected exactly one ToolCallRequested, got: {events:?}"
        );
        match tool_calls[0] {
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => {
                assert_eq!(tool_call_id, "call_abc");
                assert_eq!(tool_id, "shell.exec");
                assert_eq!(arguments["command"], "ls");
            }
            _ => unreachable!(),
        }

        assert!(events
            .iter()
            .any(|e| matches!(e, ModelEvent::Completed { .. })));

        std::env::remove_var("TEST_KEY_OAI_TC");
    }

    #[tokio::test]
    async fn streams_multi_chunk_tool_arguments() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Simulate tool call with arguments split across multiple chunks
        // (this is how OpenAI actually streams tool calls)
        let sse_body = "data: {\"choices\": [{\"delta\": {\"tool_calls\": [{\"index\": 0, \"id\": \"call_xyz\", \"function\": {\"name\": \"fs.read\", \"arguments\": \"\"}}]}, \"index\": 0}]}\n\ndata: {\"choices\": [{\"delta\": {\"tool_calls\": [{\"index\": 0, \"function\": {\"arguments\": \"{\\\"path\\\": \\\"src/main\"}}]}, \"index\": 0}]}\n\ndata: {\"choices\": [{\"delta\": {\"tool_calls\": [{\"index\": 0, \"function\": {\"arguments\": \".rs\\\"}\"}}]}, \"index\": 0}]}\n\ndata: {\"choices\": [{\"finish_reason\": \"tool_calls\", \"index\": 0}]}\n\ndata: [DONE]\n\n";

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse_body))
            .mount(&mock_server)
            .await;

        let config = OpenAiCompatibleConfig {
            base_url: mock_server.uri(),
            api_key_env: "TEST_KEY_OAI_MC".into(),
            default_model: "test-model".into(),
            headers: vec![],
            capability_overrides: None,
        };

        std::env::set_var("TEST_KEY_OAI_MC", "test-key");
        let client = OpenAiCompatibleClient::new(config);
        let stream: BoxStream<'static, Result<ModelEvent>> = client
            .stream(ModelRequest::user_text("fast", "read main.rs"))
            .await
            .unwrap();

        let events: Vec<ModelEvent> = stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();

        let tool_calls: Vec<&ModelEvent> = events
            .iter()
            .filter(|e| matches!(e, ModelEvent::ToolCallRequested { .. }))
            .collect();
        assert_eq!(
            tool_calls.len(),
            1,
            "expected exactly one ToolCallRequested, got: {events:?}"
        );
        match tool_calls[0] {
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => {
                assert_eq!(tool_call_id, "call_xyz");
                assert_eq!(tool_id, "fs.read");
                assert_eq!(arguments["path"], "src/main.rs");
            }
            _ => unreachable!(),
        }

        std::env::remove_var("TEST_KEY_OAI_MC");
    }
}

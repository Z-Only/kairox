pub mod config;
pub mod request;
pub mod streaming;
pub mod tool_accumulator;

pub use config::AnthropicConfig;

use crate::{ModelError, ModelEvent, ModelRequest, Result};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use std::collections::HashMap;
use streaming::{parse_anthropic_raw_events, AnthropicRawEvent};
use tool_accumulator::AnthropicToolCallAccumulator;

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    config: AnthropicConfig,
    http: reqwest::Client,
}

impl AnthropicClient {
    pub fn new(config: AnthropicConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build reqwest client");
        Self { config, http }
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

    struct EnvVarGuard {
        key: &'static str,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            std::env::set_var(key, value);
            Self { key }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            std::env::remove_var(self.key);
        }
    }

    fn test_config(base_url: String, api_key_env: &'static str) -> AnthropicConfig {
        AnthropicConfig {
            base_url,
            api_key_env: api_key_env.into(),
            default_model: "test-model".into(),
            max_tokens: 4096,
            headers: Vec::new(),
            capability_overrides: None,
            temperature: None,
            top_p: None,
            top_k: None,
            extra_params: None,
        }
    }

    fn shell_tool() -> crate::ToolDefinition {
        crate::ToolDefinition {
            name: "shell.exec".into(),
            description: "Execute a shell command".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"}
                },
                "required": ["command"]
            }),
        }
    }

    #[test]
    fn builds_anthropic_messages_request() {
        let config = AnthropicConfig {
            base_url: "https://api.anthropic.com".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            default_model: "claude-sonnet-4-20250514".into(),
            max_tokens: 4096,
            headers: Vec::new(),
            capability_overrides: None,
            temperature: None,
            top_p: None,
            top_k: None,
            extra_params: None,
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
        let events = streaming::parse_anthropic_json_response(data).unwrap();
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
            temperature: None,
            top_p: None,
            top_k: None,
            extra_params: None,
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
    async fn sends_wire_request_with_auth_headers_tools_and_provider_params() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let sse_body = "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}\n\n";

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "anthropic-contract-key"))
            .and(header("anthropic-version", "2023-06-01"))
            .and(header("x-provider-contract", "anthropic"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse_body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let _env = EnvVarGuard::set("KAIROX_ANTHROPIC_CONTRACT_KEY", "anthropic-contract-key");
        let mut config = test_config(mock_server.uri(), "KAIROX_ANTHROPIC_CONTRACT_KEY");
        config.max_tokens = 2048;
        config.headers = vec![("x-provider-contract".into(), "anthropic".into())];
        config.temperature = Some(0.2);
        config.top_p = Some(0.8);
        config.top_k = Some(40);
        config.extra_params = Some(serde_json::json!({"metadata": {"contract": true}}));

        let request = ModelRequest::user_text("claude", "list files")
            .with_system_prompt("Use tools when useful.")
            .with_tools(vec![shell_tool()])
            .add_assistant_with_tools(
                "I will run ls.",
                vec![crate::ToolCall {
                    id: "toolu_contract".into(),
                    name: "shell.exec".into(),
                    arguments: serde_json::json!({"command": "ls"}),
                }],
            )
            .add_tool_result("toolu_contract", "Cargo.toml");

        let stream = AnthropicClient::new(config).stream(request).await.unwrap();
        let events = stream.collect::<Vec<_>>().await;

        assert!(events
            .iter()
            .any(|event| matches!(event, Ok(ModelEvent::Completed { .. }))));

        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value = requests[0].body_json().unwrap();
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["max_tokens"], 2048);
        assert_eq!(body["stream"], true);
        assert_eq!(body["system"], "Use tools when useful.");
        assert!((body["temperature"].as_f64().unwrap() - 0.2).abs() < 1e-6);
        assert!((body["top_p"].as_f64().unwrap() - 0.8).abs() < 1e-6);
        assert_eq!(body["top_k"], 40);
        assert_eq!(body["metadata"]["contract"], true);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "list files");
        assert_eq!(body["messages"][1]["role"], "assistant");
        assert_eq!(body["messages"][1]["content"][0]["type"], "text");
        assert_eq!(body["messages"][1]["content"][0]["text"], "I will run ls.");
        assert_eq!(body["messages"][1]["content"][1]["type"], "tool_use");
        assert_eq!(body["messages"][1]["content"][1]["id"], "toolu_contract");
        assert_eq!(body["messages"][1]["content"][1]["name"], "shell_exec");
        assert_eq!(body["messages"][1]["content"][1]["input"]["command"], "ls");
        assert_eq!(body["messages"][2]["role"], "user");
        assert_eq!(body["messages"][2]["content"][0]["type"], "tool_result");
        assert_eq!(
            body["messages"][2]["content"][0]["tool_use_id"],
            "toolu_contract"
        );
        assert_eq!(body["messages"][2]["content"][0]["content"], "Cargo.toml");
        assert_eq!(body["tools"][0]["name"], "shell_exec");
        assert_eq!(body["tools"][0]["input_schema"]["required"][0], "command");
    }

    #[tokio::test]
    async fn missing_api_key_returns_request_error_before_http() {
        let key = "KAIROX_ANTHROPIC_MISSING_CONTRACT_KEY";
        std::env::remove_var(key);

        let result = AnthropicClient::new(test_config("http://127.0.0.1:1".into(), key))
            .stream(ModelRequest::user_text("claude", "hello"))
            .await;
        let err = match result {
            Err(err) => err,
            Ok(_) => panic!("expected missing API key error"),
        };

        match err {
            ModelError::Request(message) => {
                assert!(message.contains("Anthropic API key not set"), "{message}");
            }
            other => panic!("expected ModelError::Request, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn maps_http_error_status_and_body_to_api_error() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(529).set_body_string(r#"{"error":{"message":"overloaded"}}"#),
            )
            .mount(&mock_server)
            .await;

        let _env = EnvVarGuard::set("KAIROX_ANTHROPIC_HTTP_ERROR_KEY", "test-key");
        let result = AnthropicClient::new(test_config(
            mock_server.uri(),
            "KAIROX_ANTHROPIC_HTTP_ERROR_KEY",
        ))
        .stream(ModelRequest::user_text("claude", "hello"))
        .await;
        let err = match result {
            Err(err) => err,
            Ok(_) => panic!("expected HTTP error"),
        };

        match err {
            ModelError::Api(message) => {
                assert!(message.contains("529"), "{message}");
                assert!(message.contains("overloaded"), "{message}");
            }
            other => panic!("expected ModelError::Api, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn surfaces_malformed_sse_payload_as_stream_parse_error() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string("data: not-json\n\n"))
            .mount(&mock_server)
            .await;

        let _env = EnvVarGuard::set("KAIROX_ANTHROPIC_PARSE_ERROR_KEY", "test-key");
        let mut stream = AnthropicClient::new(test_config(
            mock_server.uri(),
            "KAIROX_ANTHROPIC_PARSE_ERROR_KEY",
        ))
        .stream(ModelRequest::user_text("claude", "hello"))
        .await
        .unwrap();

        let first = stream
            .next()
            .await
            .expect("stream should yield parse error");
        assert!(matches!(first, Err(ModelError::StreamParse(_))));
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
            temperature: None,
            top_p: None,
            top_k: None,
            extra_params: None,
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
            temperature: None,
            top_p: None,
            top_k: None,
            extra_params: None,
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

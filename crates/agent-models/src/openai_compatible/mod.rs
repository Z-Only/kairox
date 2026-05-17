pub mod config;
pub mod request;
pub mod streaming;
pub mod tool_accumulator;

pub use config::OpenAiCompatibleConfig;

use crate::{ModelError, ModelEvent, ModelRequest, Result};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use streaming::{parse_openai_chunk, OpenAiChunkEvent};
use tool_accumulator::OpenAiToolCallAccumulator;

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleClient {
    config: OpenAiCompatibleConfig,
    http: reqwest::Client,
}

impl OpenAiCompatibleClient {
    pub fn new(config: OpenAiCompatibleConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build reqwest client");
        Self { config, http }
    }

    pub fn from_config(config: OpenAiCompatibleConfig, http: reqwest::Client) -> Self {
        Self { config, http }
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

    fn test_config(base_url: String, api_key_env: &'static str) -> OpenAiCompatibleConfig {
        OpenAiCompatibleConfig {
            base_url,
            api_key_env: api_key_env.into(),
            default_model: "test-model".into(),
            headers: vec![],
            capability_overrides: None,
            temperature: None,
            top_p: None,
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
    fn builds_chat_request_with_system_prompt_and_messages() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.openai.com/v1".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            default_model: "gpt-4.1".into(),
            headers: vec![],
            capability_overrides: None,
            temperature: None,
            top_p: None,
            extra_params: None,
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
            temperature: None,
            top_p: None,
            extra_params: None,
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
            temperature: None,
            top_p: None,
            extra_params: None,
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
            temperature: None,
            top_p: None,
            extra_params: None,
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
            temperature: None,
            top_p: None,
            extra_params: None,
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
    async fn sends_wire_request_with_auth_headers_tools_and_provider_params() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let sse_body =
            "data: {\"choices\":[{\"finish_reason\":\"stop\",\"index\":0}]}\n\ndata: [DONE]\n\n";

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("authorization", "Bearer openai-contract-key"))
            .and(header("x-provider-contract", "openai"))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse_body))
            .expect(1)
            .mount(&mock_server)
            .await;

        let _env = EnvVarGuard::set("KAIROX_OPENAI_CONTRACT_KEY", "openai-contract-key");
        let mut config = test_config(mock_server.uri(), "KAIROX_OPENAI_CONTRACT_KEY");
        config.headers = vec![("x-provider-contract".into(), "openai".into())];
        config.temperature = Some(0.2);
        config.top_p = Some(0.8);
        config.extra_params = Some(serde_json::json!({"seed": 42}));

        let request = ModelRequest::user_text("fast", "list files")
            .with_system_prompt("Use tools when useful.")
            .with_tools(vec![shell_tool()])
            .add_assistant_with_tools(
                "I will run ls.",
                vec![crate::ToolCall {
                    id: "call_contract".into(),
                    name: "shell.exec".into(),
                    arguments: serde_json::json!({"command": "ls"}),
                }],
            )
            .add_tool_result("call_contract", "Cargo.toml");

        let stream = OpenAiCompatibleClient::new(config)
            .stream(request)
            .await
            .unwrap();
        let events = stream.collect::<Vec<_>>().await;

        assert!(events
            .iter()
            .any(|event| matches!(event, Ok(ModelEvent::Completed { .. }))));

        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value = requests[0].body_json().unwrap();
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["stream"], true);
        assert!((body["temperature"].as_f64().unwrap() - 0.2).abs() < 1e-6);
        assert!((body["top_p"].as_f64().unwrap() - 0.8).abs() < 1e-6);
        assert_eq!(body["seed"], 42);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "Use tools when useful.");
        assert_eq!(body["messages"][2]["role"], "assistant");
        assert_eq!(body["messages"][2]["content"], "I will run ls.");
        assert_eq!(body["messages"][2]["tool_calls"][0]["id"], "call_contract");
        assert_eq!(
            body["messages"][2]["tool_calls"][0]["function"]["name"],
            "shell.exec"
        );
        assert_eq!(
            body["messages"][2]["tool_calls"][0]["function"]["arguments"],
            "{\"command\":\"ls\"}"
        );
        assert_eq!(body["messages"][3]["role"], "tool");
        assert_eq!(body["messages"][3]["tool_call_id"], "call_contract");
        assert_eq!(body["messages"][3]["content"], "Cargo.toml");
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["function"]["name"], "shell.exec");
        assert_eq!(
            body["tools"][0]["function"]["parameters"]["required"][0],
            "command"
        );
    }

    #[tokio::test]
    async fn maps_http_error_status_and_body_to_api_error() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_string(r#"{"error":{"message":"rate limit exceeded"}}"#),
            )
            .mount(&mock_server)
            .await;

        let result = OpenAiCompatibleClient::new(test_config(
            mock_server.uri(),
            "KAIROX_OPENAI_HTTP_ERROR_KEY",
        ))
        .stream(ModelRequest::user_text("fast", "hello"))
        .await;
        let err = match result {
            Err(err) => err,
            Ok(_) => panic!("expected HTTP error"),
        };

        match err {
            ModelError::Api(message) => {
                assert!(message.contains("429"), "{message}");
                assert!(message.contains("rate limit exceeded"), "{message}");
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
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string("data: not-json\n\n"))
            .mount(&mock_server)
            .await;

        let mut stream = OpenAiCompatibleClient::new(test_config(
            mock_server.uri(),
            "KAIROX_OPENAI_PARSE_ERROR_KEY",
        ))
        .stream(ModelRequest::user_text("fast", "hello"))
        .await
        .unwrap();

        let first = stream
            .next()
            .await
            .expect("stream should yield parse error");
        assert!(matches!(first, Err(ModelError::StreamParse(_))));
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
            temperature: None,
            top_p: None,
            extra_params: None,
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
            temperature: None,
            top_p: None,
            extra_params: None,
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

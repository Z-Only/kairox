use super::*;
use crate::{ModelClient, ModelError, ModelEvent, ModelRequest, Result};
use futures::stream::BoxStream;
use futures::StreamExt;

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
    assert_eq!(tools[0]["function"]["name"], "fs_read");
}

#[test]
fn builds_multimodal_chat_request_from_markdown_image_data_uri() {
    let client = OpenAiCompatibleClient::new(OpenAiCompatibleConfig {
        base_url: "https://api.openai.com/v1".into(),
        api_key_env: "OPENAI_API_KEY".into(),
        default_model: "gpt-4.1".into(),
        headers: vec![],
        capability_overrides: None,
        temperature: None,
        top_p: None,
        extra_params: None,
    });
    let request = ModelRequest::user_text(
        "fast",
        "![fixture.png](data:image/png;base64,AQIDBA==)\n\nRead the code.",
    );

    let body = client.build_chat_request(&request).unwrap();

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "user");
    let content = messages[0]["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "image_url");
    assert_eq!(
        content[0]["image_url"]["url"],
        "data:image/png;base64,AQIDBA=="
    );
    assert_eq!(content[1]["type"], "text");
    assert_eq!(content[1]["text"], "Read the code.");
    assert!(!content[1]["text"].as_str().unwrap().contains("base64"));
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
    assert_eq!(tool_calls[0]["function"]["name"], "shell_exec");

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
fn omits_empty_content_only_chat_messages() {
    let client = OpenAiCompatibleClient::new(OpenAiCompatibleConfig {
        base_url: "https://api.openai.com/v1".into(),
        api_key_env: "OPENAI_API_KEY".into(),
        default_model: "gpt-4.1".into(),
        headers: vec![],
        capability_overrides: None,
        temperature: None,
        top_p: None,
        extra_params: None,
    });
    let request = ModelRequest::user_text("fast", "hello")
        .with_system_prompt("  \n\t  ")
        .add_message("assistant", "")
        .add_message("user", "   ")
        .add_message("assistant", "answer");

    let body = client.build_chat_request(&request).unwrap();

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"], "hello");
    assert_eq!(messages[1]["role"], "assistant");
    assert_eq!(messages[1]["content"], "answer");
    assert!(
        messages.iter().all(|message| !message["content"]
            .as_str()
            .is_some_and(|text| text.trim().is_empty())),
        "empty plain text messages should not be sent to provider gateways"
    );
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
        .with_reasoning_effort("xhigh")
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
    assert_eq!(body["reasoning_effort"], "xhigh");
    assert_eq!(body["seed"], 42);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "Use tools when useful.");
    assert_eq!(body["messages"][2]["role"], "assistant");
    assert_eq!(body["messages"][2]["content"], "I will run ls.");
    assert_eq!(body["messages"][2]["tool_calls"][0]["id"], "call_contract");
    assert_eq!(
        body["messages"][2]["tool_calls"][0]["function"]["name"],
        "shell_exec"
    );
    assert_eq!(
        body["messages"][2]["tool_calls"][0]["function"]["arguments"],
        "{\"command\":\"ls\"}"
    );
    assert_eq!(body["messages"][3]["role"], "tool");
    assert_eq!(body["messages"][3]["tool_call_id"], "call_contract");
    assert_eq!(body["messages"][3]["content"], "Cargo.toml");
    assert_eq!(body["tools"][0]["type"], "function");
    assert_eq!(body["tools"][0]["function"]["name"], "shell_exec");
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
        ModelError::Api { status, message } => {
            assert_eq!(status, 429);
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
async fn maps_sanitized_wire_tool_call_names_to_internal_tool_ids() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let sse_body = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_sanitized\",\"function\":{\"name\":\"shell_exec\",\"arguments\":\"{\\\"command\\\":\\\"ls\\\"}\"}}]},\"index\":0}]}\n\ndata: {\"choices\":[{\"finish_reason\":\"tool_calls\",\"index\":0}]}\n\ndata: [DONE]\n\n";

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse_body))
        .mount(&mock_server)
        .await;

    let _env = EnvVarGuard::set("TEST_KEY_OAI_SANITIZED_TC", "test-key");
    let client =
        OpenAiCompatibleClient::new(test_config(mock_server.uri(), "TEST_KEY_OAI_SANITIZED_TC"));
    let stream = client
        .stream(ModelRequest::user_text("fast", "list files").with_tools(vec![shell_tool()]))
        .await
        .unwrap();

    let events: Vec<ModelEvent> = stream
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();
    let tool_call = events
        .iter()
        .find_map(|event| match event {
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => Some((tool_call_id, tool_id, arguments)),
            _ => None,
        })
        .expect("expected ToolCallRequested");

    assert_eq!(tool_call.0, "call_sanitized");
    assert_eq!(tool_call.1, "shell.exec");
    assert_eq!(tool_call.2["command"], "ls");
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

#[test]
fn tool_result_with_screenshot_data_uri_becomes_multimodal() {
    let client = OpenAiCompatibleClient::new(OpenAiCompatibleConfig {
        base_url: "https://api.openai.com/v1".into(),
        api_key_env: "OPENAI_API_KEY".into(),
        default_model: "gpt-4.1".into(),
        headers: vec![],
        capability_overrides: None,
        temperature: None,
        top_p: None,
        extra_params: None,
    });

    let request = ModelRequest::user_text("fast", "take a screenshot")
        .with_tools(vec![shell_tool()])
        .add_assistant_with_tools(
            "",
            vec![crate::ToolCall {
                id: "call_ss".into(),
                name: "computer_use".into(),
                arguments: serde_json::json!({"action": "screenshot"}),
            }],
        )
        .add_tool_result(
            "call_ss",
            "success: true\noutput: Screenshot captured\nscreenshot:\n![screenshot](data:image/png;base64,AQIDBA==)",
        );

    let body = client.build_chat_request(&request).unwrap();
    let messages = body["messages"].as_array().unwrap();

    // Find the tool result message
    let tool_msg = messages
        .iter()
        .find(|m| m["role"] == "tool")
        .expect("should have a tool result message");

    // Content should be a multimodal array, not a plain string
    let content = tool_msg["content"]
        .as_array()
        .expect("tool result content should be a multimodal array");

    // Should have text + image_url parts
    let has_text = content.iter().any(|p| p["type"] == "text");
    let has_image = content.iter().any(|p| p["type"] == "image_url");
    assert!(has_text, "should have a text content part");
    assert!(has_image, "should have an image_url content part");

    // The image_url should contain the data URI
    let image_part = content.iter().find(|p| p["type"] == "image_url").unwrap();
    assert!(image_part["image_url"]["url"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));
}

#[test]
fn tool_result_without_images_stays_plain_string() {
    let client = OpenAiCompatibleClient::new(OpenAiCompatibleConfig {
        base_url: "https://api.openai.com/v1".into(),
        api_key_env: "OPENAI_API_KEY".into(),
        default_model: "gpt-4.1".into(),
        headers: vec![],
        capability_overrides: None,
        temperature: None,
        top_p: None,
        extra_params: None,
    });

    let request = ModelRequest::user_text("fast", "run ls")
        .with_tools(vec![shell_tool()])
        .add_assistant_with_tools(
            "",
            vec![crate::ToolCall {
                id: "call_ls".into(),
                name: "shell_exec".into(),
                arguments: serde_json::json!({"command": "ls"}),
            }],
        )
        .add_tool_result("call_ls", "file1.txt\nfile2.rs");

    let body = client.build_chat_request(&request).unwrap();
    let messages = body["messages"].as_array().unwrap();

    let tool_msg = messages
        .iter()
        .find(|m| m["role"] == "tool")
        .expect("should have a tool result message");

    // Content should be a plain string, not an array
    assert!(
        tool_msg["content"].is_string(),
        "tool result without images should be a plain string"
    );
}

use super::*;
use crate::ModelClient;

#[test]
fn builds_ollama_chat_request() {
    let config = OllamaConfig::default();
    let client = OllamaClient::new(config);
    let request = ModelRequest::user_text("local-code", "explain this")
        .with_system_prompt("You are a code assistant.");

    let body = client.build_chat_request(&request);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(body["model"], "llama3");
    assert_eq!(body["stream"], true);
}

#[test]
fn parses_ollama_ndjson_token_line() {
    let line = r#"{"message":{"role":"assistant","content":"Hello"},"done":false}"#;
    let event = parse_ollama_line(line).unwrap();
    assert_eq!(event, Some(ModelEvent::TokenDelta("Hello".into())));
}

#[test]
fn parses_ollama_done_line() {
    let line = r#"{"message":{"role":"assistant","content":""},"done":true}"#;
    let event = parse_ollama_line(line).unwrap();
    assert!(matches!(event, Some(ModelEvent::Completed { usage: None })));
}

#[test]
fn ollama_usage_parses_prompt_and_eval_counts_from_done_line() {
    let line = r#"{"message":{"role":"assistant","content":""},"done":true,"prompt_eval_count":12,"eval_count":34}"#;
    let event = parse_ollama_line(line).unwrap();

    assert_eq!(
        event,
        Some(ModelEvent::Completed {
            usage: Some(crate::ModelUsage {
                input_tokens: 12,
                output_tokens: 34,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            })
        })
    );
}

#[tokio::test]
async fn streams_from_wiremock_server() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    let ndjson_body = format!(
        "{}\n{}\n{}\n",
        r#"{"message":{"role":"assistant","content":"Hi"},"done":false}"#,
        r#"{"message":{"role":"assistant","content":" there"},"done":false}"#,
        r#"{"message":{"role":"assistant","content":""},"done":true}"#
    );

    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string(ndjson_body))
        .mount(&mock_server)
        .await;

    let config = OllamaConfig {
        base_url: mock_server.uri(),
        default_model: "test-model".into(),
        context_window: 4096,
    };
    let client = OllamaClient::new(config);
    let mut stream = client
        .stream(ModelRequest::user_text("local", "hello"))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event.unwrap());
    }

    assert!(events
        .iter()
        .any(|e| matches!(e, ModelEvent::TokenDelta(t) if t == "Hi")));
    assert!(events
        .iter()
        .any(|e| matches!(e, ModelEvent::TokenDelta(t) if t == " there")));
    assert!(events
        .iter()
        .any(|e| matches!(e, ModelEvent::Completed { .. })));
}

#[tokio::test]
async fn probe_context_window_reads_context_length_from_show_endpoint() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let body = serde_json::json!({
        "model_info": {
            "general.architecture": "llama",
            "llama.context_length": 8192_u64
        }
    });

    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&mock_server)
        .await;

    let client = OllamaClient::new(OllamaConfig {
        base_url: mock_server.uri(),
        default_model: "llama3:8b".into(),
        context_window: 0,
    });

    assert_eq!(client.probe_context_window("llama3:8b").await, Some(8192));
}

#[tokio::test]
async fn probe_context_window_returns_none_on_http_error() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let client = OllamaClient::new(OllamaConfig {
        base_url: mock_server.uri(),
        default_model: "missing".into(),
        context_window: 0,
    });

    assert!(client.probe_context_window("missing").await.is_none());
}

#[tokio::test]
async fn probe_context_window_handles_unknown_architecture() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let body = serde_json::json!({
        "model_info": {
            "general.architecture": "qwen",
            "qwen.context_length": 32768_u64
        }
    });
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&mock_server)
        .await;

    let client = OllamaClient::new(OllamaConfig {
        base_url: mock_server.uri(),
        default_model: "qwen2:7b".into(),
        context_window: 0,
    });

    assert_eq!(client.probe_context_window("qwen2:7b").await, Some(32768));
}

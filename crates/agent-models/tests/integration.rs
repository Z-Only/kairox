use agent_models::{
    FakeModelClient, ModelCapabilities, ModelClient, ModelEvent, ModelProfile, ModelRequest,
    ModelRouter,
};
use async_trait::async_trait;
use futures::{stream, stream::BoxStream, StreamExt};
use std::sync::Arc;

/// A ModelClient that immediately fails — for testing error propagation.
#[derive(Debug, Clone)]
struct FailingModelClient {
    message: String,
}

impl FailingModelClient {
    fn new(message: &str) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[async_trait]
impl ModelClient for FailingModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        Ok(Box::pin(stream::iter(vec![Ok(ModelEvent::Failed {
            message: self.message.clone(),
        })])))
    }
}

/// A ModelClient that streams nothing then completes — for testing empty responses.
#[derive(Debug, Clone)]
struct EmptyModelClient;

#[async_trait]
impl ModelClient for EmptyModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        Ok(Box::pin(stream::iter(vec![Ok(ModelEvent::Completed {
            usage: None,
        })])))
    }
}

fn test_profile(alias: &str) -> ModelProfile {
    ModelProfile {
        alias: alias.into(),
        provider: "fake".into(),
        model_id: "test-model".into(),
        capabilities: ModelCapabilities {
            streaming: true,
            tool_calling: false,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: 4096,
            output_limit: 2048,
            local_model: true,
        },
    }
}

/// Integration test: verify that ModelRouter correctly dispatches a request
/// to the client registered under the matching profile alias.
#[tokio::test]
async fn router_selects_correct_client() {
    let mut router = ModelRouter::new();

    let gpt4_tokens = vec!["GPT-4 response".into()];
    let claude_tokens = vec!["Claude response".into()];

    router.register(
        test_profile("gpt4"),
        Arc::new(FakeModelClient::new(gpt4_tokens)),
    );
    router.register(
        test_profile("claude"),
        Arc::new(FakeModelClient::new(claude_tokens)),
    );

    // Route a request targeting the "gpt4" profile.
    let mut stream = router
        .route(ModelRequest::user_text("gpt4", "hello"))
        .await
        .unwrap();

    let mut events: Vec<ModelEvent> = Vec::new();
    while let Some(Ok(event)) = stream.next().await {
        events.push(event);
    }

    // Should get the GPT-4 tokens, not Claude tokens.
    assert!(
        events.contains(&ModelEvent::TokenDelta("GPT-4 response".into())),
        "expected GPT-4 token delta, got: {events:?}"
    );
}

/// Integration test: routing to a profile alias that was never registered
/// must return an error rather than panicking or silently succeeding.
#[tokio::test]
async fn router_falls_back_when_unknown() {
    let mut router = ModelRouter::new();

    router.register(
        test_profile("default"),
        Arc::new(FakeModelClient::new(vec!["unused".into()])),
    );

    // Request a profile that was never registered.
    let result = router
        .route(ModelRequest::user_text("nonexistent", "hello"))
        .await;

    match result {
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains("unknown model"),
                "expected 'unknown model profile' error, got: {msg}"
            );
        }
        Ok(_) => panic!("expected an error for unknown profile alias, but got Ok"),
    }
}

/// Smoke test: FakeModelClient must actually emit TokenDelta events
/// with non-empty text when streamed.
#[tokio::test]
async fn fake_model_produces_tokens() {
    let client = FakeModelClient::new(vec!["Hello".into(), " ".into(), "world".into()]);

    let mut stream = client
        .stream(ModelRequest::user_text("test", "say hello"))
        .await
        .unwrap();

    let mut events: Vec<ModelEvent> = Vec::new();
    while let Some(Ok(event)) = stream.next().await {
        events.push(event);
    }

    // Collect only TokenDelta events and verify they contain text.
    let token_texts: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            ModelEvent::TokenDelta(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();

    assert!(!token_texts.is_empty(), "expected non-empty token deltas");

    let combined: String = token_texts.iter().copied().collect();
    assert!(
        combined.len() >= 5,
        "expected combined output of at least 5 characters, got: {combined}"
    );

    // The stream should always end with a Completed event.
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ModelEvent::Completed { .. })),
        "expected a Completed event at end of stream, got: {events:?}"
    );
}

// ── Contract tests: ModelClient trait guarantees ──

/// Every ModelClient stream MUST end with a terminal event (Completed or Failed).
/// This is the most fundamental contract requirement.
#[tokio::test]
async fn contract_stream_ends_with_terminal_event() {
    // Test with FakeModelClient (success path)
    let client = FakeModelClient::new(vec!["token".into()]);
    let mut stream = client
        .stream(ModelRequest::user_text("test", "hi"))
        .await
        .unwrap();

    let mut last_event: Option<ModelEvent> = None;
    while let Some(Ok(event)) = stream.next().await {
        last_event = Some(event);
    }

    assert!(
        matches!(last_event, Some(ModelEvent::Completed { .. })),
        "stream from FakeModelClient must end with Completed, got: {last_event:?}"
    );

    // Test with FailingModelClient (error path)
    let client = FailingModelClient::new("rate limit exceeded");
    let mut stream = client
        .stream(ModelRequest::user_text("test", "hi"))
        .await
        .unwrap();

    let mut last_event: Option<ModelEvent> = None;
    while let Some(Ok(event)) = stream.next().await {
        last_event = Some(event);
    }

    assert!(
        matches!(last_event, Some(ModelEvent::Failed { .. })),
        "stream from FailingModelClient must end with Failed, got: {last_event:?}"
    );
}

/// TokenDelta events must appear in the configured order before any terminal event.
#[tokio::test]
async fn contract_token_deltas_preserve_order() {
    let tokens: Vec<String> = (0..10).map(|i| format!("t{i}")).collect();
    let client = FakeModelClient::new(tokens.clone());

    let mut stream = client
        .stream(ModelRequest::user_text("test", "hi"))
        .await
        .unwrap();

    let mut seen: Vec<String> = Vec::new();
    let mut terminal_seen = false;
    while let Some(Ok(event)) = stream.next().await {
        match event {
            ModelEvent::TokenDelta(text) => {
                assert!(
                    !terminal_seen,
                    "TokenDelta must not appear after terminal event"
                );
                seen.push(text);
            }
            ModelEvent::Completed { .. } | ModelEvent::Failed { .. } => {
                terminal_seen = true;
            }
            _ => {}
        }
    }

    assert_eq!(seen, tokens);
    assert!(terminal_seen, "stream must include a terminal event");
}

/// An empty token list should still produce a valid stream ending with Completed.
/// This guards against empty-body response panics.
#[tokio::test]
async fn contract_empty_token_stream_produces_only_completed() {
    let client = FakeModelClient::new(vec![]);
    let mut stream = client
        .stream(ModelRequest::user_text("test", "hi"))
        .await
        .unwrap();

    let mut events: Vec<ModelEvent> = Vec::new();
    while let Some(Ok(event)) = stream.next().await {
        events.push(event);
    }

    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], ModelEvent::Completed { .. }));
}

/// Client with tool_call enabled must emit ToolCallRequested after TokenDelta events
/// and before the terminal Completed event.
#[tokio::test]
async fn contract_tool_call_positioned_before_completed() {
    let client = FakeModelClient::new(vec!["reading".into()]).with_tool_call();
    let mut stream = client
        .stream(ModelRequest::user_text("test", "read"))
        .await
        .unwrap();

    let mut events: Vec<ModelEvent> = Vec::new();
    while let Some(Ok(event)) = stream.next().await {
        events.push(event);
    }

    // Expected: TokenDelta → ToolCallRequested → Completed
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], ModelEvent::TokenDelta(..)));
    assert!(matches!(events[1], ModelEvent::ToolCallRequested { .. }));
    assert!(matches!(events[2], ModelEvent::Completed { .. }));

    // Verify tool call event shape
    if let ModelEvent::ToolCallRequested {
        ref tool_call_id,
        ref tool_id,
        ref arguments,
    } = events[1]
    {
        assert!(!tool_call_id.is_empty(), "tool_call_id must not be empty");
        assert!(!tool_id.is_empty(), "tool_id must not be empty");
        assert!(arguments.is_object(), "arguments must be a JSON object");
    } else {
        panic!("expected ToolCallRequested at index 1");
    }
}

/// ModelRequest::user_text constructs a request with the given profile alias
/// and a single user message.
#[tokio::test]
async fn contract_model_request_user_text_shape() {
    let request = ModelRequest::user_text("claude", "explain this code");

    assert_eq!(request.model_profile, "claude");
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, "user");
    assert_eq!(request.messages[0].content, "explain this code");
    assert!(request.system_prompt.is_none());
}

// ── Contract tests: ModelRouter ──

/// Router with zero registered profiles must return an error for any alias.
#[tokio::test]
async fn contract_router_empty_errors_on_any_alias() {
    let router = ModelRouter::new();

    let result = router
        .route(ModelRequest::user_text("anything", "hello"))
        .await;

    assert!(result.is_err(), "empty router must error for any alias");
    assert!(
        result.err().unwrap().to_string().contains("unknown model"),
        "error must mention unknown model profile"
    );
}

/// Registering a second client under the same alias replaces the first.
#[tokio::test]
async fn contract_router_last_registration_wins() {
    let mut router = ModelRouter::new();

    router.register(
        test_profile("default"),
        Arc::new(FakeModelClient::new(vec!["first".into()])),
    );
    router.register(
        test_profile("default"),
        Arc::new(FakeModelClient::new(vec!["second".into()])),
    );

    let mut stream = router
        .route(ModelRequest::user_text("default", "hello"))
        .await
        .unwrap();

    let mut events: Vec<ModelEvent> = Vec::new();
    while let Some(Ok(event)) = stream.next().await {
        events.push(event);
    }

    // Should get "second" (last registered), not "first".
    let token_texts: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            ModelEvent::TokenDelta(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(token_texts, vec!["second"]);
}

/// Failed event must contain a non-empty message and be the final event.
#[tokio::test]
async fn contract_failed_event_contains_message() {
    let mut router = ModelRouter::new();
    router.register(
        test_profile("broken"),
        Arc::new(FailingModelClient::new("service unavailable")),
    );

    let mut stream = router
        .route(ModelRequest::user_text("broken", "hello"))
        .await
        .unwrap();

    let mut events: Vec<ModelEvent> = Vec::new();
    while let Some(Ok(event)) = stream.next().await {
        events.push(event);
    }

    assert_eq!(events.len(), 1);
    match &events[0] {
        ModelEvent::Failed { message } => {
            assert_eq!(message, "service unavailable");
            assert!(!message.is_empty());
        }
        other => panic!("expected Failed event, got: {other:?}"),
    }
}

/// Routing through a failing client at the router level must still deliver
/// the Failed event (not panic or hang).
#[tokio::test]
async fn contract_router_propagates_failed_event() {
    let mut router = ModelRouter::new();
    router.register(
        test_profile("flaky"),
        Arc::new(FailingModelClient::new("internal error")),
    );

    let mut stream = router
        .route(ModelRequest::user_text("flaky", "hello"))
        .await
        .unwrap();

    let mut events: Vec<ModelEvent> = Vec::new();
    while let Some(Ok(event)) = stream.next().await {
        events.push(event);
    }

    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], ModelEvent::Failed { .. }));
}

/// Empty response through router must propagate correctly (no phantom events).
#[tokio::test]
async fn contract_router_empty_response_produces_only_completed() {
    let mut router = ModelRouter::new();
    router.register(test_profile("mute"), Arc::new(EmptyModelClient));

    let mut stream = router
        .route(ModelRequest::user_text("mute", "hello"))
        .await
        .unwrap();

    let mut events: Vec<ModelEvent> = Vec::new();
    while let Some(Ok(event)) = stream.next().await {
        events.push(event);
    }

    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], ModelEvent::Completed { .. }));
}

/// Many single-character tokens must all be delivered without truncation.
#[tokio::test]
async fn contract_fake_many_tokens_no_truncation() {
    let tokens: Vec<String> = "Hello, world!".chars().map(|c| c.to_string()).collect();
    let count = tokens.len();
    let client = FakeModelClient::new(tokens);

    let mut stream = client
        .stream(ModelRequest::user_text("test", "hi"))
        .await
        .unwrap();

    let mut seen: Vec<String> = Vec::new();
    while let Some(Ok(event)) = stream.next().await {
        if let ModelEvent::TokenDelta(text) = event {
            seen.push(text);
        }
    }

    assert_eq!(seen.len(), count);
    let combined: String = seen.iter().cloned().collect();
    assert_eq!(combined, "Hello, world!");
}

/// Re-registering a profile should not leak tokens from the previous client.
#[tokio::test]
async fn contract_router_replace_doesnt_leak_tokens() {
    let mut router = ModelRouter::new();

    router.register(
        test_profile("default"),
        Arc::new(FakeModelClient::new(vec!["leaked".into()])),
    );
    router.register(test_profile("default"), Arc::new(EmptyModelClient));

    let mut stream = router
        .route(ModelRequest::user_text("default", "hello"))
        .await
        .unwrap();

    let mut events: Vec<ModelEvent> = Vec::new();
    while let Some(Ok(event)) = stream.next().await {
        events.push(event);
    }

    // Should be empty (only Completed), no leaked tokens from first registration.
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], ModelEvent::Completed { .. }));
    let token_texts: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            ModelEvent::TokenDelta(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(token_texts.is_empty(), "no tokens should leak");
}

/// ModelCapabilities must be independently configurable per profile.
#[tokio::test]
async fn contract_profile_capabilities_independent() {
    let mut router = ModelRouter::new();

    let reader = FakeModelClient::new(vec!["ok".into()]);
    let writer = FakeModelClient::new(vec!["done".into()]);

    let mut read_profile = test_profile("reader");
    read_profile.capabilities.streaming = true;
    read_profile.capabilities.tool_calling = false;

    let mut write_profile = test_profile("writer");
    write_profile.capabilities.streaming = true;
    write_profile.capabilities.tool_calling = true;

    router.register(read_profile, Arc::new(reader));
    router.register(write_profile, Arc::new(writer));

    // Both profiles route successfully with their own clients.
    let mut read_stream = router
        .route(ModelRequest::user_text("reader", "read"))
        .await
        .unwrap();
    let mut read_events: Vec<ModelEvent> = Vec::new();
    while let Some(Ok(event)) = read_stream.next().await {
        read_events.push(event);
    }
    assert!(
        read_events.contains(&ModelEvent::TokenDelta("ok".into())),
        "reader should produce its configured token"
    );

    let mut write_stream = router
        .route(ModelRequest::user_text("writer", "write"))
        .await
        .unwrap();
    let mut write_events: Vec<ModelEvent> = Vec::new();
    while let Some(Ok(event)) = write_stream.next().await {
        write_events.push(event);
    }
    assert!(
        write_events.contains(&ModelEvent::TokenDelta("done".into())),
        "writer should produce its configured token"
    );
}

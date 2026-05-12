use agent_models::{
    FakeModelClient, ModelCapabilities, ModelClient, ModelEvent, ModelProfile, ModelRequest,
    ModelRouter,
};
use futures::StreamExt;
use std::sync::Arc;

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
                msg.contains("unknown model profile"),
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

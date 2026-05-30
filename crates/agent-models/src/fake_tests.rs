use super::*;
use crate::{ModelEvent, ModelRequest};
use futures::StreamExt;

#[tokio::test]
async fn streams_configured_tokens_then_completion() {
    let client = FakeModelClient::new(vec!["hello".into(), " ".into(), "world".into()]);
    let mut stream = client
        .stream(ModelRequest::user_text("test", "hi"))
        .await
        .unwrap();

    let mut seen = Vec::new();
    while let Some(event) = stream.next().await {
        seen.push(event.unwrap());
    }

    assert_eq!(
        seen,
        vec![
            ModelEvent::TokenDelta("hello".into()),
            ModelEvent::TokenDelta(" ".into()),
            ModelEvent::TokenDelta("world".into()),
            ModelEvent::Completed { usage: None },
        ]
    );
}

#[tokio::test]
async fn optionally_includes_tool_call_event() {
    let client = FakeModelClient::new(vec!["reading".into()]).with_tool_call();
    let mut stream = client
        .stream(ModelRequest::user_text("test", "read"))
        .await
        .unwrap();

    let mut seen = Vec::new();
    while let Some(event) = stream.next().await {
        seen.push(event.unwrap());
    }

    assert!(matches!(&seen[1], ModelEvent::ToolCallRequested { .. }));
}

#[tokio::test]
async fn with_tool_call_for_overrides_tool_id_and_arguments() {
    let client = FakeModelClient::new(vec!["listing".into()])
        .with_tool_call_for("fs.list", serde_json::json!({"path": "."}));
    let mut stream = client
        .stream(ModelRequest::user_text("test", "ls"))
        .await
        .unwrap();

    let mut seen = Vec::new();
    while let Some(event) = stream.next().await {
        seen.push(event.unwrap());
    }

    match &seen[1] {
        ModelEvent::ToolCallRequested {
            tool_id, arguments, ..
        } => {
            assert_eq!(tool_id, "fs.list");
            assert_eq!(arguments, &serde_json::json!({"path": "."}));
        }
        other => panic!("expected ToolCallRequested, got {other:?}"),
    }
}

/// Regression: the tool call must emit only on the first stream call,
/// so the agent loop terminates after the runtime appends the tool
/// result and re-invokes the model.
#[tokio::test]
async fn tool_call_emits_only_on_first_stream_call() {
    let client = FakeModelClient::new(vec!["listing".into()])
        .with_tool_call_for("fs.list", serde_json::json!({"path": "."}));

    let mut first_stream = client
        .stream(ModelRequest::user_text("test", "ls"))
        .await
        .unwrap();
    let mut first = Vec::new();
    while let Some(event) = first_stream.next().await {
        first.push(event.unwrap());
    }
    let first_tool_calls = first
        .iter()
        .filter(|e| matches!(e, ModelEvent::ToolCallRequested { .. }))
        .count();
    assert_eq!(
        first_tool_calls, 1,
        "first stream should emit one tool call"
    );

    let mut second_stream = client
        .stream(ModelRequest::user_text("test", "ls"))
        .await
        .unwrap();
    let mut second = Vec::new();
    while let Some(event) = second_stream.next().await {
        second.push(event.unwrap());
    }
    let second_tool_calls = second
        .iter()
        .filter(|e| matches!(e, ModelEvent::ToolCallRequested { .. }))
        .count();
    assert_eq!(
        second_tool_calls, 0,
        "subsequent stream calls should not re-emit the tool call",
    );
}

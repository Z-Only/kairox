//! Plain text turns (no tool calls) and the loop-iteration guard constant.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

/// Verify MAX_AGENT_LOOP_ITERATIONS is a reasonable value — the constant
/// guards against infinite loops, so it must be positive and bounded.
#[test]
#[allow(clippy::assertions_on_constants)]
fn max_agent_loop_iterations_is_reasonable() {
    use agent_runtime::agent_loop::MAX_AGENT_LOOP_ITERATIONS;
    assert!(MAX_AGENT_LOOP_ITERATIONS > 0);
    assert!(MAX_AGENT_LOOP_ITERATIONS <= 100);
}

#[tokio::test]
async fn agent_loop_stops_when_no_tool_calls() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Just a text response".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-no-tools".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(projection.messages.len(), 2);
    assert_eq!(projection.messages[0].content, "hello");
    assert_eq!(projection.messages[1].content, "Just a text response");
}

/// Verify the exact event sequence for a simple (non-tool-call) completion.
/// Key events must appear in the expected relative order.
#[tokio::test]
async fn agent_loop_emits_completion_event_sequence() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Short reply".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/test-event-seq".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    let event_types: Vec<String> = trace.iter().map(|e| e.event.event_type.clone()).collect();

    // Verify key events exist
    assert!(
        event_types.contains(&"UserMessageAdded".to_string()),
        "Missing UserMessageAdded: {:?}",
        event_types
    );
    assert!(
        event_types.contains(&"ModelTokenDelta".to_string()),
        "Missing ModelTokenDelta: {:?}",
        event_types
    );
    assert!(
        event_types.contains(&"AssistantMessageCompleted".to_string()),
        "Missing AssistantMessageCompleted: {:?}",
        event_types
    );
    assert!(
        event_types.contains(&"AgentTaskCompleted".to_string()),
        "Missing AgentTaskCompleted: {:?}",
        event_types
    );

    // Verify expected relative order
    let user_pos = event_types
        .iter()
        .position(|t| t == "UserMessageAdded")
        .unwrap();
    let assistant_pos = event_types
        .iter()
        .position(|t| t == "AssistantMessageCompleted")
        .unwrap();
    let completed_pos = event_types
        .iter()
        .position(|t| t == "AgentTaskCompleted")
        .unwrap();

    assert!(
        user_pos < assistant_pos,
        "UserMessageAdded should come before AssistantMessageCompleted"
    );
    assert!(
        assistant_pos < completed_pos,
        "AssistantMessageCompleted should come before AgentTaskCompleted"
    );
}

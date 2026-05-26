//! End-to-end workspace → session → message → projection → cancel → trace test.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_store::SqliteEventStore;

use super::support::make_runtime;

#[tokio::test]
async fn full_workspace_session_round_trip() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);

    // Open workspace
    let workspace = runtime
        .open_workspace("/tmp/kairox-round-trip".into())
        .await
        .unwrap();

    // Start session
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    // Send message
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "hello agent".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Get projection — should have 2 messages (user + assistant)
    let projection = runtime
        .get_session_projection(session_id.clone())
        .await
        .unwrap();
    assert_eq!(
        projection.messages.len(),
        2,
        "expected 2 messages (user + assistant), got {:?}",
        projection
            .messages
            .iter()
            .map(|m| format!("{:?}: {}", m.role, m.content))
            .collect::<Vec<_>>()
    );
    assert_eq!(projection.messages[0].content, "hello agent");
    assert_eq!(projection.messages[1].content, "response");

    // Cancel session
    runtime
        .cancel_session(workspace.workspace_id, session_id.clone())
        .await
        .unwrap();

    // Verify cancelled flag
    let projection_after_cancel = runtime
        .get_session_projection(session_id.clone())
        .await
        .unwrap();
    assert!(
        projection_after_cancel.cancelled,
        "session should be marked as cancelled"
    );

    // Get trace — should be non-empty
    let trace = runtime.get_trace(session_id).await.unwrap();
    assert!(
        !trace.is_empty(),
        "trace should contain events after the round trip"
    );
}

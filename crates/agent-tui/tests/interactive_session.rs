//! Integration tests for the full session flow — runtime + event subscription
//! WITHOUT requiring a terminal (no crossterm TTY).

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use futures::StreamExt;

#[tokio::test]
async fn full_session_flow_sends_message_and_receives_response() {
    // Setup: in-memory event store + fake model + LocalRuntime with Suggest mode
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hello from fake model".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

    // Open workspace and start session
    let workspace = runtime
        .open_workspace("/tmp/kairox-test-session".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Send a user message
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hi".into(),
        })
        .await
        .unwrap();

    // Get session projection and verify
    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(
        projection.messages.len(),
        2,
        "expected 2 messages (user + assistant), got {:?}",
        projection.messages
    );
    assert_eq!(projection.messages[0].content, "hi");
    assert_eq!(projection.messages[1].content, "hello from fake model");
}

#[tokio::test]
async fn event_subscription_receives_streaming_events() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hello from fake model".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace("/tmp/kairox-test-subscribe".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Subscribe to session events BEFORE sending the message
    let mut event_stream = runtime.subscribe_session(session_id.clone());

    // Send message
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hi".into(),
        })
        .await
        .unwrap();

    // Collect events from stream with timeout
    let mut received_events = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);

    loop {
        tokio::select! {
            event = event_stream.next() => {
                match event {
                    Some(e) => {
                        received_events.push(e);
                        if received_events.len() > 20 {
                            break;
                        }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                break;
            }
        }
    }

    assert!(
        !received_events.is_empty(),
        "subscribe_session should receive at least 1 event, got 0"
    );

    // Verify we got at least UserMessageAdded
    let event_types: Vec<String> = received_events
        .iter()
        .map(|e| e.event_type.to_string())
        .collect();
    assert!(
        event_types.iter().any(|t| t == "UserMessageAdded"),
        "expected UserMessageAdded in event types, got: {:?}",
        event_types
    );
}

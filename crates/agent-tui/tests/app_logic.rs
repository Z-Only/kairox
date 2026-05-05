//! TUI App logic integration tests.
//!
//! These tests verify the TUI's core logic (command dispatch, event handling,
//! state transitions) WITHOUT requiring a real terminal. They use the
//! FakeModelClient + in-memory event store to exercise the full
//! LocalRuntime → App event pipeline.

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use futures::StreamExt;

/// Helper: create a runtime with FakeModelClient.
async fn make_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Hello from TUI test!".into()]);
    LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest)
}

// ---------------------------------------------------------------------------
// Test: Workspace → Session → SendMessage → Projection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_send_message_produces_user_and_assistant_messages() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-test-workspace".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello from TUI".into(),
        })
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(
        projection.messages.len(),
        2,
        "Expected user + assistant messages"
    );
    assert_eq!(projection.messages[0].role, ProjectedRole::User);
    assert_eq!(projection.messages[0].content, "hello from TUI");
    assert_eq!(projection.messages[1].role, ProjectedRole::Assistant);
    assert_eq!(projection.messages[1].content, "Hello from TUI test!");
}

// ---------------------------------------------------------------------------
// Test: Event stream mirrors projection data
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_event_stream_matches_projection() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-event-test".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    let mut event_stream = runtime.subscribe_session(session_id.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "test events".into(),
        })
        .await
        .unwrap();

    // Collect events from stream
    let mut received_events = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        tokio::select! {
            event = event_stream.next() => {
                match event {
                    Some(e) => received_events.push(e),
                    None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    assert!(
        !received_events.is_empty(),
        "Should receive at least one event"
    );

    let event_types: Vec<&str> = received_events
        .iter()
        .map(|e| e.event_type.as_str())
        .collect();

    assert!(
        event_types.contains(&"UserMessageAdded"),
        "Expected UserMessageAdded in events: {event_types:?}"
    );
    assert!(
        event_types.contains(&"AssistantMessageCompleted"),
        "Expected AssistantMessageCompleted in events: {event_types:?}"
    );
}

// ---------------------------------------------------------------------------
// Test: Multiple sessions, projection isolation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_multiple_sessions_have_isolated_projections() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["response".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace("/tmp/tui-multi-session".into())
        .await
        .unwrap();

    let s1 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();
    let s2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Send to s1 only
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: s1.clone(),
            content: "message for s1".into(),
        })
        .await
        .unwrap();

    let proj1 = runtime.get_session_projection(s1).await.unwrap();
    let proj2 = runtime.get_session_projection(s2).await.unwrap();

    assert_eq!(proj1.messages.len(), 2, "Session 1 should have 2 messages");
    assert_eq!(proj2.messages.len(), 0, "Session 2 should have 0 messages");
}

// ---------------------------------------------------------------------------
// Test: Session cancellation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_cancel_session_marks_cancelled() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["response".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace("/tmp/tui-cancel".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    runtime
        .cancel_session(workspace.workspace_id, session_id.clone())
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert!(
        projection.cancelled,
        "Session should be marked as cancelled"
    );
}

// ---------------------------------------------------------------------------
// Test: Trace entries are populated
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_trace_entries_populated_after_message() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-trace".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "trace me".into(),
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    assert!(
        !trace.is_empty(),
        "Trace should have entries after a message"
    );

    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    assert!(
        event_types.contains(&"UserMessageAdded"),
        "Trace should contain UserMessageAdded: {event_types:?}"
    );
}

// ---------------------------------------------------------------------------
// Test: Session listing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_session_listing_works() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-list".into())
        .await
        .unwrap();

    runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();
    runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "smart".into(),
        })
        .await
        .unwrap();

    let sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 2);
}

// ---------------------------------------------------------------------------
// Test: Subscribe-all receives events across sessions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_subscribe_all_receives_events_across_sessions() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hello".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace("/tmp/tui-sub-all".into())
        .await
        .unwrap();

    let s1 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();
    let s2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    let mut all_stream = runtime.subscribe_all();

    // Send to both sessions
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: s1.clone(),
            content: "msg1".into(),
        })
        .await
        .unwrap();
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: s2.clone(),
            content: "msg2".into(),
        })
        .await
        .unwrap();

    // Collect events
    let mut session_ids = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(1000);
    loop {
        tokio::select! {
            event = all_stream.next() => {
                match event {
                    Some(e) => {
                        session_ids.push(e.session_id.to_string());
                        if session_ids.len() > 20 { break; }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    assert!(
        session_ids.contains(&s1.to_string()),
        "subscribe_all should receive events from session 1"
    );
    assert!(
        session_ids.contains(&s2.to_string()),
        "subscribe_all should receive events from session 2"
    );
}

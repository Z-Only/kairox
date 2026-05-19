//! Refactor baseline tests for LocalRuntime.
//!
//! These tests anchor the current behavior so we can verify zero regression
//! after each module extraction step.  They intentionally cover the core
//! `AppFacade` surface with minimal setup (in-memory SQLite + FakeModelClient).

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest, WorkspaceId};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use futures::StreamExt;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn make_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["assistant reply".into()]);
    LocalRuntime::new(store, model)
}

/// Common setup: open a workspace and start a session, returning the runtime,
/// workspace ID, and session ID.
async fn setup_session() -> (
    LocalRuntime<SqliteEventStore, FakeModelClient>,
    WorkspaceId,
    agent_core::SessionId,
) {
    let runtime = make_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/kairox-baseline".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();
    (runtime, ws.workspace_id, sid)
}

// ---------------------------------------------------------------------------
// 1. send_message records both user and assistant events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn baseline_send_message_records_user_and_assistant_events() {
    let (runtime, _ws, sid) = setup_session().await;

    runtime
        .send_message(SendMessageRequest {
            workspace_id: _ws.clone(),
            session_id: sid.clone(),
            content: "hello baseline".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let projection = runtime.get_session_projection(sid).await.unwrap();
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
    assert_eq!(projection.messages[0].content, "hello baseline");
    assert_eq!(projection.messages[1].content, "assistant reply");
}

// ---------------------------------------------------------------------------
// 2. open_workspace persists and list_workspaces returns it
// ---------------------------------------------------------------------------

#[tokio::test]
async fn baseline_open_workspace_persists_and_lists() {
    let runtime = make_runtime().await;

    let ws = runtime
        .open_workspace("/tmp/kairox-ws-list".into())
        .await
        .unwrap();

    let workspaces = runtime.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1, "should list exactly 1 workspace");
    assert_eq!(
        workspaces[0].workspace_id, ws.workspace_id,
        "listed workspace ID should match the created one"
    );
    assert_eq!(workspaces[0].path, "/tmp/kairox-ws-list");
}

// ---------------------------------------------------------------------------
// 3. Session lifecycle: start → list → rename → soft_delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn baseline_session_lifecycle() {
    let runtime = make_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/kairox-lifecycle".into())
        .await
        .unwrap();

    // start
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    // list — should contain the new session
    let sessions = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert_eq!(sessions.len(), 1, "should have 1 session after start");
    assert_eq!(sessions[0].session_id, sid);

    // rename
    runtime
        .rename_session(&sid, "Baseline Renamed".into())
        .await
        .unwrap();
    let after_rename = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert_eq!(after_rename[0].title, "Baseline Renamed");

    // soft-delete
    runtime.soft_delete_session(&sid).await.unwrap();
    let after_delete = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert!(
        after_delete.is_empty(),
        "soft-deleted session should not appear in active list"
    );
}

// ---------------------------------------------------------------------------
// 4. cancel_session emits event without error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn baseline_cancel_session_emits_event() {
    let (runtime, ws, sid) = setup_session().await;

    runtime
        .cancel_session(ws, sid.clone())
        .await
        .expect("cancel_session should succeed without error");

    let projection = runtime.get_session_projection(sid).await.unwrap();
    assert!(
        projection.cancelled,
        "session should be marked as cancelled after cancel_session"
    );
}

// ---------------------------------------------------------------------------
// 5. subscribe_session receives events after send_message
// ---------------------------------------------------------------------------

#[tokio::test]
async fn baseline_subscribe_receives_events() {
    let (runtime, ws, sid) = setup_session().await;

    let mut stream = runtime.subscribe_session(sid.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws,
            session_id: sid.clone(),
            content: "subscribe test".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Collect events with a timeout so the test doesn't hang.
    let mut received = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        tokio::select! {
            event = stream.next() => {
                match event {
                    Some(e) => {
                        received.push(e);
                        if received.len() > 30 { break; }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    assert!(
        !received.is_empty(),
        "subscribe_session should yield events after send_message"
    );
    let event_types: Vec<&str> = received.iter().map(|e| e.event_type.as_str()).collect();
    assert!(
        event_types.contains(&"UserMessageAdded"),
        "event stream should contain UserMessageAdded, got: {event_types:?}"
    );
}

// ---------------------------------------------------------------------------
// 6. get_task_graph returns empty for a new session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn baseline_task_graph_initially_empty() {
    let (runtime, _ws, sid) = setup_session().await;

    let snapshot = runtime.get_task_graph(sid).await.unwrap();
    assert!(
        snapshot.tasks.is_empty(),
        "task graph should be empty for a brand-new session that has not yet sent a message"
    );
}

// ---------------------------------------------------------------------------
// 7. get_trace returns events after send_message
// ---------------------------------------------------------------------------

#[tokio::test]
async fn baseline_trace_returns_session_events() {
    let (runtime, ws, sid) = setup_session().await;

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws,
            session_id: sid.clone(),
            content: "trace test".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(sid).await.unwrap();
    assert!(
        !trace.is_empty(),
        "trace should contain events after send_message"
    );
    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();
    assert!(
        event_types.contains(&"UserMessageAdded"),
        "trace should contain UserMessageAdded, got: {event_types:?}"
    );
    assert!(
        event_types.contains(&"AssistantMessageCompleted"),
        "trace should contain AssistantMessageCompleted, got: {event_types:?}"
    );
}

// ---------------------------------------------------------------------------
// 8. Turn emits ContextAssembled event with usage data
// ---------------------------------------------------------------------------

#[tokio::test]
async fn baseline_turn_emits_context_assembled_event() {
    let (runtime, ws, sid) = setup_session().await;

    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws,
            session_id: sid.clone(),
            content: "context assembly check".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(sid).await.unwrap();
    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();
    assert!(
        event_types.contains(&"ContextAssembled"),
        "trace should contain ContextAssembled after turn context preparation, got: {event_types:?}"
    );

    // ContextAssembled must appear after UserMessageAdded and before
    // the first ModelTokenDelta (i.e., context is prepared before the
    // model is called).
    let user_pos = event_types.iter().position(|t| *t == "UserMessageAdded");
    let assembled_pos = event_types.iter().position(|t| *t == "ContextAssembled");

    assert!(
        user_pos.is_some() && assembled_pos.is_some(),
        "both UserMessageAdded and ContextAssembled must exist"
    );
    assert!(
        user_pos.unwrap() < assembled_pos.unwrap(),
        "ContextAssembled must appear after UserMessageAdded"
    );
    if let Some(delta_pos) = event_types.iter().position(|t| *t == "ModelTokenDelta") {
        assert!(
            assembled_pos.unwrap() < delta_pos,
            "ContextAssembled must appear before first ModelTokenDelta"
        );
    }
}

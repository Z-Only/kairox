use super::*;
use agent_store::SqliteEventStore;
use futures::StreamExt;

async fn build_store() -> SqliteEventStore {
    SqliteEventStore::in_memory().await.unwrap()
}

fn make_event_tx() -> tokio::sync::broadcast::Sender<DomainEvent> {
    tokio::sync::broadcast::channel(100).0
}

// ---------------------------------------------------------------------------
// SessionState::default
// ---------------------------------------------------------------------------

#[test]
fn session_state_default_fields() {
    let state = SessionState::default();
    assert!(state.model_limits.is_none());
    assert_eq!(state.last_estimated_tokens, 0);
    assert!(!state.compacting);
}

// ---------------------------------------------------------------------------
// temporary_title_from_first_message
// ---------------------------------------------------------------------------

#[test]
fn title_empty_string_gives_new_conversation() {
    assert_eq!(temporary_title_from_first_message(""), "New conversation");
}

#[test]
fn title_short_text_preserved() {
    assert_eq!(
        temporary_title_from_first_message("Hello world"),
        "Hello world"
    );
}

#[test]
fn title_truncated_at_48_chars_with_ellipsis() {
    let long_input = "a".repeat(60);
    let result = temporary_title_from_first_message(&long_input);
    // 48 chars + 1 ellipsis char
    assert_eq!(result.chars().count(), 49);
    assert!(result.ends_with('…'));
    assert_eq!(&result[..48], &"a".repeat(48));
}

#[test]
fn title_whitespace_trimmed() {
    assert_eq!(
        temporary_title_from_first_message("  hi there  "),
        "hi there"
    );
}

#[test]
fn title_only_whitespace_gives_new_conversation() {
    assert_eq!(
        temporary_title_from_first_message("   "),
        "New conversation"
    );
}

// ---------------------------------------------------------------------------
// open_workspace
// ---------------------------------------------------------------------------

#[tokio::test]
async fn open_workspace_returns_matching_path() {
    let store = build_store().await;
    let event_tx = make_event_tx();

    let info = open_workspace(&store, &event_tx, "/tmp/project".into())
        .await
        .unwrap();

    assert_eq!(info.path, "/tmp/project");
}

#[tokio::test]
async fn open_workspace_broadcasts_event() {
    let store = build_store().await;
    let event_tx = make_event_tx();
    let mut rx = event_tx.subscribe();

    let _info = open_workspace(&store, &event_tx, "/tmp/ws".into())
        .await
        .unwrap();

    let event = rx.try_recv().unwrap();
    assert!(
        matches!(event.payload, EventPayload::WorkspaceOpened { ref path } if path == "/tmp/ws")
    );
}

// ---------------------------------------------------------------------------
// start_session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn start_session_returns_valid_session_id() {
    let store = build_store().await;
    let event_tx = make_event_tx();
    let workspace_id = WorkspaceId::new();

    let session_id = start_session(&store, &event_tx, workspace_id, "gpt-4".into(), None, None)
        .await
        .unwrap();

    // SessionId should produce a non-empty string
    assert!(!session_id.to_string().is_empty());
}

#[tokio::test]
async fn start_session_persists_metadata() {
    let store = build_store().await;
    let event_tx = make_event_tx();
    let workspace_id = WorkspaceId::new();

    // Must upsert workspace first so list_active_sessions can find it
    store
        .upsert_workspace(&workspace_id.to_string(), "/tmp/test")
        .await
        .unwrap();

    let _session_id = start_session(
        &store,
        &event_tx,
        workspace_id.clone(),
        "claude-sonnet".into(),
        None,
        None,
    )
    .await
    .unwrap();

    let sessions = store
        .list_active_sessions(&workspace_id.to_string())
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].model_profile, "claude-sonnet");
}

// ---------------------------------------------------------------------------
// derive_policy_strings
// ---------------------------------------------------------------------------

#[test]
fn derive_policy_strings_defaults() {
    let (approval, sandbox) = derive_policy_strings(None, None);
    // Should produce non-empty default strings
    assert!(!approval.is_empty());
    assert!(!sandbox.is_empty());
}

#[test]
fn derive_policy_strings_respects_overrides() {
    // Pass a known valid override for approval
    let (approval, _sandbox) = derive_policy_strings(Some("never".into()), None);
    assert_eq!(approval, "never");
}

// ---------------------------------------------------------------------------
// cancel_session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cancel_session_emits_session_cancelled() {
    let store = build_store().await;
    let event_tx = make_event_tx();
    let mut rx = event_tx.subscribe();

    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    cancel_session(&store, &event_tx, workspace_id, session_id)
        .await
        .unwrap();

    let event = rx.try_recv().unwrap();
    assert!(matches!(
        event.payload,
        EventPayload::SessionCancelled { .. }
    ));
}

// ---------------------------------------------------------------------------
// get_trace
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_trace_empty_session_returns_empty_vec() {
    let store = build_store().await;
    let session_id = SessionId::new();

    let trace = get_trace(&store, session_id).await.unwrap();
    assert!(trace.is_empty());
}

#[tokio::test]
async fn get_trace_with_events_returns_entries() {
    let store = build_store().await;
    let event_tx = make_event_tx();

    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    // Append an event directly
    let event = DomainEvent::new(
        workspace_id,
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::SessionCancelled {
            reason: "test".into(),
        },
    );
    append_and_broadcast(&store, &event_tx, &event)
        .await
        .unwrap();

    let trace = get_trace(&store, session_id).await.unwrap();
    assert_eq!(trace.len(), 1);
    assert!(matches!(
        trace[0].event.payload,
        EventPayload::SessionCancelled { .. }
    ));
}

// ---------------------------------------------------------------------------
// subscribe_session / subscribe_all
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscribe_session_filters_by_session_id() {
    let event_tx = make_event_tx();
    let target_session = SessionId::new();
    let other_session = SessionId::new();

    let stream = subscribe_session(&event_tx, target_session.clone());

    // Send events for two different sessions
    let target_event = DomainEvent::new(
        WorkspaceId::new(),
        target_session.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::SessionCancelled {
            reason: "target".into(),
        },
    );
    let other_event = DomainEvent::new(
        WorkspaceId::new(),
        other_session,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::SessionCancelled {
            reason: "other".into(),
        },
    );

    event_tx.send(target_event).unwrap();
    event_tx.send(other_event).unwrap();
    drop(event_tx); // close channel so stream ends

    let received: Vec<_> = stream.collect().await;
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].session_id, target_session);
}

#[tokio::test]
async fn subscribe_all_receives_all_events() {
    let event_tx = make_event_tx();
    let stream = subscribe_all(&event_tx);

    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::SessionCancelled {
            reason: "all".into(),
        },
    );
    event_tx.send(event).unwrap();
    drop(event_tx);

    let received: Vec<_> = stream.collect().await;
    assert_eq!(received.len(), 1);
}

// ---------------------------------------------------------------------------
// list_workspaces
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_workspaces_empty_store() {
    let store = build_store().await;
    let workspaces = list_workspaces(&store).await.unwrap();
    assert!(workspaces.is_empty());
}

#[tokio::test]
async fn list_workspaces_after_open() {
    let store = build_store().await;
    let event_tx = make_event_tx();

    let info = open_workspace(&store, &event_tx, "/tmp/listed".into())
        .await
        .unwrap();

    let workspaces = list_workspaces(&store).await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].workspace_id, info.workspace_id);
    assert_eq!(workspaces[0].path, "/tmp/listed");
}

// ---------------------------------------------------------------------------
// rename_session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rename_session_updates_title() {
    let store = build_store().await;
    let event_tx = make_event_tx();
    let workspace_id = WorkspaceId::new();

    store
        .upsert_workspace(&workspace_id.to_string(), "/tmp/rename")
        .await
        .unwrap();

    let session_id = start_session(
        &store,
        &event_tx,
        workspace_id.clone(),
        "test-model".into(),
        None,
        None,
    )
    .await
    .unwrap();

    rename_session(&store, &session_id, "My Cool Title".into())
        .await
        .unwrap();

    let sessions = store
        .list_active_sessions(&workspace_id.to_string())
        .await
        .unwrap();
    assert_eq!(sessions[0].title, "My Cool Title");
}

// ---------------------------------------------------------------------------
// soft_delete_session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn soft_delete_session_marks_deleted() {
    let store = build_store().await;
    let event_tx = make_event_tx();
    let workspace_id = WorkspaceId::new();

    store
        .upsert_workspace(&workspace_id.to_string(), "/tmp/del")
        .await
        .unwrap();

    let session_id = start_session(
        &store,
        &event_tx,
        workspace_id.clone(),
        "model".into(),
        None,
        None,
    )
    .await
    .unwrap();

    soft_delete_session(&store, &session_id).await.unwrap();

    // After soft-delete, list_active_sessions should not return it
    let sessions = store
        .list_active_sessions(&workspace_id.to_string())
        .await
        .unwrap();
    assert!(sessions.is_empty());
}

// ---------------------------------------------------------------------------
// get_task_graph
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_task_graph_no_graph_returns_default() {
    let task_graphs: Arc<Mutex<HashMap<String, TaskGraph>>> = Arc::new(Mutex::new(HashMap::new()));
    let session_id = SessionId::new();

    let snapshot = get_task_graph(&task_graphs, session_id).await.unwrap();
    assert!(snapshot.tasks.is_empty());
}

// ---------------------------------------------------------------------------
// session_row_to_meta
// ---------------------------------------------------------------------------

#[test]
fn session_row_to_meta_maps_all_fields() {
    let row = SessionRow {
        session_id: "sid-123".into(),
        workspace_id: "wid-456".into(),
        title: "Test Session".into(),
        model_profile: "gpt-4o".into(),
        model_id: Some("gpt-4o-2024".into()),
        provider: Some("openai".into()),
        approval_policy: Some("never".into()),
        sandbox_policy: Some("{}".into()),
        deleted_at: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-02T00:00:00Z".into(),
    };

    let meta = session_row_to_meta(row);

    assert_eq!(meta.session_id.to_string(), "sid-123");
    assert_eq!(meta.workspace_id.to_string(), "wid-456");
    assert_eq!(meta.title, "Test Session");
    assert_eq!(meta.model_profile, "gpt-4o");
    assert_eq!(meta.model_id.as_deref(), Some("gpt-4o-2024"));
    assert_eq!(meta.provider.as_deref(), Some("openai"));
    assert_eq!(meta.approval_policy.as_deref(), Some("never"));
    assert_eq!(meta.sandbox_policy.as_deref(), Some("{}"));
    assert!(meta.deleted_at.is_none());
    assert_eq!(meta.created_at, "2026-01-01T00:00:00Z");
    assert_eq!(meta.updated_at, "2026-01-02T00:00:00Z");
    // Fields not populated from SessionRow
    assert!(meta.project_id.is_none());
    assert!(meta.worktree_path.is_none());
    assert!(meta.branch.is_none());
    assert!(meta.visibility.is_none());
}

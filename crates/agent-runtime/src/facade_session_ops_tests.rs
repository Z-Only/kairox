use super::*;
use agent_core::facade::SessionFacade;
use agent_core::StartSessionRequest;
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use agent_tools::ApprovalPolicy;

async fn build_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(store, model)
}

async fn build_runtime_with_response(
    response: &str,
) -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![response.into()]);
    LocalRuntime::new(store, model)
}

// ---------- open_workspace ----------

#[tokio::test]
async fn open_workspace_returns_workspace_info_and_persists() {
    let runtime = build_runtime().await;
    let info = runtime.open_workspace("/tmp/test-ws".into()).await.unwrap();

    assert_eq!(info.path, "/tmp/test-ws");
    assert!(!info.workspace_id.as_str().is_empty());

    let workspaces = runtime.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].workspace_id, info.workspace_id);
}

// ---------- start_session ----------

#[tokio::test]
async fn start_session_returns_id_and_persists_metadata() {
    let runtime = build_runtime().await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    assert!(!session_id.as_str().is_empty());

    let sessions = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, session_id);
}

#[tokio::test]
async fn start_session_with_approval_policy() {
    let runtime = build_runtime().await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let _session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: Some("always".into()),
            sandbox_policy: None,
        })
        .await
        .unwrap();

    // Verify the approval policy was parsed and set
    let engine = runtime.permission_engine.lock().await;
    assert_eq!(engine.approval_policy(), ApprovalPolicy::Always);
}

#[tokio::test]
async fn start_session_with_sandbox_policy() {
    let runtime = build_runtime().await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let _session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: Some(r#"{"kind":"read_only"}"#.into()),
        })
        .await
        .unwrap();

    let engine = runtime.permission_engine.lock().await;
    assert!(matches!(
        engine.sandbox_policy(),
        agent_tools::SandboxPolicy::ReadOnly
    ));
}

// ---------- send_message ----------

#[tokio::test]
async fn send_message_rejects_when_compacting() {
    let runtime = build_runtime_with_response("hi").await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    // Manually set compacting flag via the test accessor
    {
        let states = runtime.session_states_for_test().lock().await;
        // We need to drop and re-acquire with insert
        drop(states);
        let mut states = runtime.session_states_for_test().lock().await;
        states.insert(
            session_id.to_string(),
            crate::session::SessionState {
                model_limits: None,
                usage_corrector: Default::default(),
                last_estimated_tokens: 0,
                compacting: true,
            },
        );
    }

    let result = runtime
        .send_message(agent_core::SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id,
            content: "hello".into(),
            display_content: None,
            attachments: vec![],
        })
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("compaction in progress"));
}

// ---------- cancel_session ----------

#[tokio::test]
async fn cancel_session_emits_session_cancelled_event() {
    let runtime = build_runtime_with_response("hi").await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .cancel_session(ws.workspace_id.clone(), session_id.clone())
        .await
        .unwrap();

    let events = runtime.get_trace(session_id).await.unwrap();
    let has_cancelled = events
        .iter()
        .any(|e| e.event.event_type == "SessionCancelled");
    assert!(has_cancelled, "expected SessionCancelled event in trace");
}

// ---------- get_trace ----------

#[tokio::test]
async fn get_trace_returns_stored_events() {
    let runtime = build_runtime_with_response("hello").await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(agent_core::SendMessageRequest {
            workspace_id: ws.workspace_id,
            session_id: session_id.clone(),
            content: "hi".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    assert!(!trace.is_empty());
}

// ---------- list_workspaces ----------

#[tokio::test]
async fn list_workspaces_returns_opened_workspaces() {
    let runtime = build_runtime().await;

    runtime.open_workspace("/tmp/a".into()).await.unwrap();
    runtime.open_workspace("/tmp/b".into()).await.unwrap();

    let workspaces = runtime.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 2);

    let paths: Vec<&str> = workspaces.iter().map(|w| w.path.as_str()).collect();
    assert!(paths.contains(&"/tmp/a"));
    assert!(paths.contains(&"/tmp/b"));
}

// ---------- list_sessions ----------

#[tokio::test]
async fn list_sessions_scoped_to_workspace() {
    let runtime = build_runtime().await;

    let ws1 = runtime.open_workspace("/tmp/ws1".into()).await.unwrap();
    let ws2 = runtime.open_workspace("/tmp/ws2".into()).await.unwrap();

    let sid1 = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws1.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let _sid2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws2.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let sessions_ws1 = runtime.list_sessions(&ws1.workspace_id).await.unwrap();
    assert_eq!(sessions_ws1.len(), 1);
    assert_eq!(sessions_ws1[0].session_id, sid1);
}

// ---------- rename_session ----------

#[tokio::test]
async fn rename_session_updates_title() {
    let runtime = build_runtime().await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .rename_session(&session_id, "New Title".into())
        .await
        .unwrap();

    let sessions = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert_eq!(sessions[0].title, "New Title");
}

// ---------- soft_delete_session ----------

#[tokio::test]
async fn soft_delete_session_marks_deleted() {
    let runtime = build_runtime().await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime.soft_delete_session(&session_id).await.unwrap();

    // After soft delete, list_sessions should not return the session
    let sessions = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert!(
        sessions.is_empty() || sessions.iter().all(|s| s.session_id != session_id),
        "soft-deleted session should not appear in list"
    );
}

// ---------- permanently_delete_session ----------

#[tokio::test]
async fn permanently_delete_session_removes_completely() {
    let runtime = build_runtime().await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .permanently_delete_session(&session_id)
        .await
        .unwrap();

    let sessions = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert!(sessions.is_empty());
}

// ---------- restore_archived_session ----------

#[tokio::test]
async fn restore_archived_session_makes_visible_again() {
    let runtime = build_runtime().await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime.soft_delete_session(&session_id).await.unwrap();

    // Verify it's gone
    let sessions = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert!(sessions.iter().all(|s| s.session_id != session_id));

    // Restore
    runtime.restore_archived_session(&session_id).await.unwrap();

    let sessions = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert!(sessions.iter().any(|s| s.session_id == session_id));
}

// ---------- get_task_graph ----------

#[tokio::test]
async fn get_task_graph_returns_empty_when_no_graph() {
    let runtime = build_runtime().await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let snapshot = runtime.get_task_graph(session_id).await.unwrap();
    assert!(snapshot.tasks.is_empty());
}

// ---------- get_agent_status ----------

#[tokio::test]
async fn get_agent_status_returns_empty_without_dag_executor() {
    let runtime = build_runtime().await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let statuses = runtime.get_agent_status(session_id).await.unwrap();
    assert!(statuses.is_empty());
}

// ---------- list_trajectories ----------

#[tokio::test]
async fn list_trajectories_returns_empty_without_trajectory_store() {
    let runtime = build_runtime().await;
    let ws = runtime.open_workspace("/tmp/ws".into()).await.unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let trajectories = runtime.list_trajectories(session_id).await.unwrap();
    assert!(trajectories.is_empty());
}

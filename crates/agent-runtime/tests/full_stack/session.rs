//! Session lifecycle: start, rename, soft-delete.

use agent_core::{AppFacade, StartSessionRequest};

use super::support::make_simple_runtime;

#[tokio::test]
async fn full_stack_start_session_under_workspace() {
    let runtime = make_simple_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-session".into())
        .await
        .unwrap();

    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    assert!(
        sid.as_str().starts_with("ses_"),
        "Session ID should have ses_ prefix, got: {}",
        sid.as_str()
    );

    let sessions = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, sid);
}

#[tokio::test]
async fn full_stack_rename_and_soft_delete_session() {
    let runtime = make_simple_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-rename-delete".into())
        .await
        .unwrap();
    let sid = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    // Rename
    runtime
        .rename_session(&sid, "New Name".into())
        .await
        .unwrap();
    let sessions = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert_eq!(sessions[0].title, "New Name");

    // Soft-delete
    runtime.soft_delete_session(&sid).await.unwrap();
    let after_delete = runtime.list_sessions(&ws.workspace_id).await.unwrap();
    assert!(
        after_delete.is_empty(),
        "Soft-deleted session should not appear in active list"
    );
}

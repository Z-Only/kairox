//! Rename and soft-delete behavior for sessions inside a single workspace.

use agent_core::{AppFacade, StartSessionRequest};
use agent_store::SqliteEventStore;

use super::support::make_runtime;

#[tokio::test]
async fn rename_and_soft_delete_flow() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);

    // Open workspace → start session
    let workspace = runtime
        .open_workspace("/tmp/kairox-rename-delete".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    // Rename
    runtime
        .rename_session(&session_id, "Renamed Session".into())
        .await
        .unwrap();

    // Verify new title in list
    let sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "Renamed Session");

    // Soft-delete
    runtime.soft_delete_session(&session_id).await.unwrap();

    // Verify list is empty (soft-deleted sessions are excluded from active list)
    let sessions_after_delete = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert!(
        sessions_after_delete.is_empty(),
        "soft-deleted session should not appear in active list"
    );
}

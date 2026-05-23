//! Archival behavior when a project is removed while sessions are visible.

use agent_core::AppFacade;
use agent_store::SqliteEventStore;

use super::support::make_runtime;

#[tokio::test]
async fn project_removal_archives_visible_project_sessions() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-workspace".into())
        .await
        .unwrap();

    let project = runtime
        .add_existing_project(workspace.workspace_id.clone(), "/tmp/kairox-project".into())
        .await
        .unwrap();
    let session_id = runtime
        .create_project_draft_session(project.project_id.clone())
        .await
        .unwrap();

    runtime
        .mark_session_visible(&session_id, "hello project".into())
        .await
        .unwrap();
    runtime
        .remove_project(project.project_id.clone())
        .await
        .unwrap();

    let visible = runtime
        .list_project_sessions(project.project_id.clone())
        .await
        .unwrap();
    let archived = runtime
        .list_archived_sessions(&workspace.workspace_id)
        .await
        .unwrap();

    assert!(visible.is_empty());
    assert!(archived
        .iter()
        .any(|session| session.session_id == session_id));
}

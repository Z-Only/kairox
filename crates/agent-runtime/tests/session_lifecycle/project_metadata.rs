//! Project-session list metadata and SQLite-store contract checks.

use agent_core::{AppFacade, CoreError, ProjectId, ProjectSessionVisibility, WorkspaceId};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

use super::support::{make_runtime, NonSqliteEventStore};

#[tokio::test]
async fn project_session_lists_require_sqlite_metadata_store() {
    let runtime = LocalRuntime::new(
        NonSqliteEventStore,
        FakeModelClient::new(vec!["response".into()]),
    );

    let visible_error = runtime
        .list_project_sessions(ProjectId::from_string("prj_non_sqlite".into()))
        .await
        .expect_err("non-SQLite project session listing should fail");
    let archived_error = runtime
        .list_archived_sessions(&WorkspaceId::from_string("wrk_non_sqlite".into()))
        .await
        .expect_err("non-SQLite archived session listing should fail");

    for error in [visible_error, archived_error] {
        match error {
            CoreError::InvalidState(message) => {
                assert_eq!(message, "project metadata requires sqlite event store")
            }
            other => panic!("expected InvalidState, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn facade_projects_lists_draft_sessions_with_project_metadata() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-facade-projects-workspace".into())
        .await
        .unwrap();
    let project_root = tempfile::tempdir().expect("temp project root");
    let project_root_string = project_root.path().display().to_string();
    let project = runtime
        .add_existing_project(workspace.workspace_id.clone(), project_root_string.clone())
        .await
        .unwrap();

    let session_id = runtime
        .create_project_draft_session(project.project_id.clone())
        .await
        .unwrap();

    let project_sessions = runtime
        .list_project_sessions(project.project_id.clone())
        .await
        .unwrap();
    let workspace_sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();

    assert!(
        workspace_sessions.is_empty(),
        "project-bound sessions should not duplicate in workspace session list"
    );
    assert_eq!(project_sessions.len(), 1);
    let session = &project_sessions[0];
    assert_eq!(session.session_id, session_id);
    assert_eq!(session.project_id, Some(project.project_id));
    assert_eq!(session.worktree_path, Some(project_root_string));
    assert_eq!(
        session.visibility,
        Some(ProjectSessionVisibility::DraftHidden)
    );
}

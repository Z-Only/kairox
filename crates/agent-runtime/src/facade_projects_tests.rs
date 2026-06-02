use super::*;
use crate::execution_runtime::ExecutionState;
use agent_core::AppFacade;
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;

#[tokio::test]
async fn create_project_draft_session_registers_idle_actor() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hello".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/kairox-workspace".into())
        .await
        .unwrap();
    let project = runtime
        .add_existing_project(workspace.workspace_id, "/tmp/kairox-project".into())
        .await
        .unwrap();

    assert_eq!(runtime.session_execution.actor_count().await, 0);
    let session_id = runtime
        .create_project_draft_session(project.project_id)
        .await
        .unwrap();

    assert_eq!(runtime.session_execution.actor_count().await, 1);
    assert_eq!(
        runtime.session_execution.session_state(&session_id).await,
        Some(ExecutionState::Idle)
    );
}

#[tokio::test]
async fn list_project_branches_returns_empty_for_non_git_project() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hello".into()]);
    let runtime = LocalRuntime::new(store, model);
    let project_root = tempfile::tempdir().expect("project root");

    let workspace = runtime
        .open_workspace(project_root.path().display().to_string())
        .await
        .unwrap();
    let project = runtime
        .add_existing_project(
            workspace.workspace_id,
            project_root.path().display().to_string(),
        )
        .await
        .unwrap();

    let branches = runtime
        .list_project_branches(project.project_id)
        .await
        .unwrap();

    assert!(branches.is_empty());
}

#[tokio::test]
async fn project_removal_stops_archived_session_actor_and_restore_restarts_it() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hello".into()]);
    let runtime = LocalRuntime::new(store, model);

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
    assert_eq!(
        runtime.session_execution.session_state(&session_id).await,
        Some(ExecutionState::Idle)
    );

    runtime
        .remove_project(project.project_id.clone())
        .await
        .unwrap();

    assert_eq!(
        runtime.session_execution.session_state(&session_id).await,
        None
    );
    assert_eq!(runtime.session_execution.actor_count().await, 0);
    let archived = runtime
        .list_archived_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert!(archived
        .iter()
        .any(|session| session.session_id == session_id));

    runtime
        .restore_project_session(session_id.clone())
        .await
        .unwrap();

    assert_eq!(runtime.session_execution.actor_count().await, 1);
    assert_eq!(
        runtime.session_execution.session_state(&session_id).await,
        Some(ExecutionState::Idle)
    );
}

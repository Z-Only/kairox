//! `create_blank_project` default name and git-init failure handling.

use agent_core::{AppFacade, CoreError};
use agent_store::SqliteEventStore;

use super::support::{make_runtime, ENV_LOCK};

#[tokio::test]
async fn create_blank_project_uses_new_project_default_name() {
    let _environment_guard = ENV_LOCK.lock().await;
    let previous_home = std::env::var_os("HOME");
    let home_dir = tempfile::tempdir().expect("temp home");

    std::env::set_var("HOME", home_dir.path());

    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-blank-project-default-name".into())
        .await
        .unwrap();
    let project = runtime
        .create_blank_project(workspace.workspace_id, None)
        .await
        .unwrap();

    match previous_home {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }

    assert_eq!(project.display_name, "New Project");
}

#[tokio::test]
async fn create_blank_project_reports_git_init_failure() {
    let _environment_guard = ENV_LOCK.lock().await;
    let previous_home = std::env::var_os("HOME");
    let previous_path = std::env::var_os("PATH");
    let home_dir = tempfile::tempdir().expect("temp home");

    std::env::set_var("HOME", home_dir.path());
    std::env::set_var("PATH", "");

    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-blank-project-git-failure".into())
        .await
        .unwrap();
    let result = runtime
        .create_blank_project(workspace.workspace_id, Some("No Git Available".into()))
        .await;

    match previous_home {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }
    match previous_path {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let error = result.expect_err("missing git executable should fail blank project creation");
    assert!(
        matches!(error, CoreError::InvalidState(_)),
        "expected InvalidState, got {error:?}"
    );
}

//! `create_project_worktree_session` happy path and branch-validation failure.

use agent_core::AppFacade;
use agent_store::SqliteEventStore;

use super::support::{make_runtime, ENV_LOCK};

#[tokio::test]
async fn create_project_worktree_session_creates_isolated_worktree() {
    let _environment_guard = ENV_LOCK.lock().await;
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-worktree-session".into())
        .await
        .unwrap();

    let temp = tempfile::tempdir().expect("temp project root");
    let project_root = temp.path().display().to_string();

    // init a git repo with a commit so worktree add has a starting point
    let git_init = std::process::Command::new("git")
        .args(["-C", &project_root, "init"])
        .output()
        .expect("git init");
    assert!(git_init.status.success(), "git init should succeed");

    // create an initial commit so there's something to branch from
    std::fs::write(temp.path().join("README.md"), "hello").unwrap();
    let git_add = std::process::Command::new("git")
        .args(["-C", &project_root, "add", "README.md"])
        .output()
        .expect("git add");
    assert!(git_add.status.success());
    let git_commit = std::process::Command::new("git")
        .args([
            "-C",
            &project_root,
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@test",
            "commit",
            "-m",
            "initial",
        ])
        .output()
        .expect("git commit");
    assert!(git_commit.status.success(), "git commit should succeed");

    // create the branch we'll check out in the worktree
    let git_branch = std::process::Command::new("git")
        .args(["-C", &project_root, "branch", "feat-demo"])
        .output()
        .expect("git branch");
    assert!(git_branch.status.success(), "git branch should succeed");

    let project = runtime
        .add_existing_project(workspace.workspace_id.clone(), project_root.clone())
        .await
        .unwrap();

    let session_id = runtime
        .create_project_worktree_session(project.project_id.clone(), "feat-demo".into())
        .await
        .unwrap();

    // verify the worktree was actually created on disk
    let expected_worktree = std::path::Path::new(&project_root).join(".kairox/worktrees/feat-demo");
    assert!(
        expected_worktree.exists(),
        "worktree dir should exist at {:?}",
        expected_worktree
    );
    assert!(
        expected_worktree.join(".git").exists(),
        "worktree should have a .git file"
    );

    // verify the session is bound to the worktree path (not the project root)
    let sessions = runtime
        .list_project_sessions(project.project_id.clone())
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
    let session_meta = &sessions[0];
    assert_eq!(session_meta.session_id, session_id);
    assert_eq!(
        session_meta.worktree_path,
        Some(expected_worktree.display().to_string())
    );
    assert_eq!(session_meta.branch, Some("feat-demo".to_string()));

    // cleanup: remove the worktree (git worktree remove)
    let _ = std::process::Command::new("git")
        .args([
            "-C",
            &project_root,
            "worktree",
            "remove",
            &expected_worktree.display().to_string(),
            "--force",
        ])
        .output();
}

#[tokio::test]
async fn create_project_worktree_session_fails_on_nonexistent_branch() {
    let _environment_guard = ENV_LOCK.lock().await;
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-worktree-fail".into())
        .await
        .unwrap();

    let temp = tempfile::tempdir().expect("temp project root");
    let project_root = temp.path().display().to_string();

    let git_init = std::process::Command::new("git")
        .args(["-C", &project_root, "init"])
        .output()
        .expect("git init");
    assert!(git_init.status.success());

    // create an initial commit
    std::fs::write(temp.path().join("README.md"), "hello").unwrap();
    let _ = std::process::Command::new("git")
        .args(["-C", &project_root, "add", "README.md"])
        .output()
        .expect("git add");
    let _ = std::process::Command::new("git")
        .args([
            "-C",
            &project_root,
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@test",
            "commit",
            "-m",
            "initial",
        ])
        .output()
        .expect("git commit");

    let project = runtime
        .add_existing_project(workspace.workspace_id.clone(), project_root)
        .await
        .unwrap();

    let result = runtime
        .create_project_worktree_session(project.project_id, "nonexistent-branch".into())
        .await;

    assert!(
        result.is_err(),
        "worktree session should fail for nonexistent branch"
    );
}

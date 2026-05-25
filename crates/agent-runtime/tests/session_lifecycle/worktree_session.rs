//! `create_project_worktree_session` happy path and new branch creation.

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
async fn create_project_worktree_session_creates_and_checks_out_new_branch() {
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
        .add_existing_project(workspace.workspace_id.clone(), project_root.clone())
        .await
        .unwrap();

    let session_id = runtime
        .create_project_worktree_session(project.project_id.clone(), "new-feature".into())
        .await;

    assert!(
        session_id.is_ok(),
        "worktree session should create a missing branch"
    );
    let expected_worktree =
        std::path::Path::new(&project_root).join(".kairox/worktrees/new-feature");
    let branch_output = std::process::Command::new("git")
        .args([
            "-C",
            &expected_worktree.display().to_string(),
            "branch",
            "--show-current",
        ])
        .output()
        .expect("git branch --show-current");
    assert!(branch_output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&branch_output.stdout).trim(),
        "new-feature"
    );

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
async fn list_project_branches_returns_local_branches() {
    let _environment_guard = ENV_LOCK.lock().await;
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-branch-list".into())
        .await
        .unwrap();

    let temp = tempfile::tempdir().expect("temp project root");
    let project_root = temp.path().display().to_string();
    assert!(std::process::Command::new("git")
        .args(["-C", &project_root, "init"])
        .output()
        .expect("git init")
        .status
        .success());
    std::fs::write(temp.path().join("README.md"), "hello").unwrap();
    assert!(std::process::Command::new("git")
        .args(["-C", &project_root, "add", "README.md"])
        .output()
        .expect("git add")
        .status
        .success());
    assert!(std::process::Command::new("git")
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
        .expect("git commit")
        .status
        .success());
    assert!(std::process::Command::new("git")
        .args(["-C", &project_root, "branch", "feat/chat"])
        .output()
        .expect("git branch")
        .status
        .success());
    let project = runtime
        .add_existing_project(workspace.workspace_id.clone(), project_root)
        .await
        .unwrap();

    let branches = runtime
        .list_project_branches(project.project_id)
        .await
        .expect("branches should load");

    assert!(branches.iter().any(|branch| branch == "feat/chat"));
    assert!(
        branches
            .iter()
            .any(|branch| branch == "main" || branch == "master"),
        "default git branch should be included: {branches:?}"
    );
}

//! Integration tests for agent settings resolution through DagExecutor.

mod support;

use agent_core::AgentRole;
use agent_runtime::AgentSettingsRoots;
use support::dag_executor::{make_executor_with_roots, write_agent_settings};

#[tokio::test]
async fn agent_settings_model_profile_override() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    write_agent_settings(
        &ws_agents,
        "default",
        "Custom default",
        "Custom planner instructions.",
        Some("fast"),
        &[],
        true,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let overrides = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");
    assert_eq!(overrides.0.as_deref(), Some("fast"));
    assert!(overrides.1.is_none());
    assert!(overrides.2.is_empty());
    assert!(overrides.3.is_empty());
}

#[tokio::test]
async fn agent_settings_reasoning_effort_override() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    tokio::fs::create_dir_all(&ws_agents).await.unwrap();
    tokio::fs::write(
        ws_agents.join("worker.md"),
        "---\nname: worker\ndescription: Custom worker\nreasoning_effort: high\ntools: []\n---\nWorker instructions.\n",
    )
    .await
    .unwrap();

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let overrides = executor
        .agent_settings_overrides(AgentRole::Worker)
        .expect("worker strategy must exist");
    assert_eq!(overrides.1.as_deref(), Some("high"));
}

#[tokio::test]
async fn agent_settings_project_overrides_user() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    write_agent_settings(
        &usr_agents,
        "default",
        "User default",
        "User instructions.",
        Some("slow"),
        &["fs.read"],
        true,
    )
    .await;
    write_agent_settings(
        &ws_agents,
        "default",
        "Project default",
        "Project instructions.",
        Some("fast"),
        &["search"],
        true,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let overrides = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");
    assert_eq!(
        overrides.0.as_deref(),
        Some("fast"),
        "project model_profile should override user"
    );
    assert_eq!(
        overrides.3,
        vec!["search"],
        "project tools should override user"
    );
}

#[tokio::test]
async fn agent_settings_disabled_project_falls_back_to_user() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    write_agent_settings(
        &usr_agents,
        "default",
        "User default",
        "User instructions.",
        Some("balanced"),
        &["fs.read"],
        true,
    )
    .await;
    write_agent_settings(
        &ws_agents,
        "default",
        "Disabled project default",
        "Project instructions.",
        Some("fast"),
        &["search"],
        false,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let overrides = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");
    assert_eq!(overrides.0.as_deref(), Some("balanced"));
    assert_eq!(overrides.3, vec!["fs.read"]);
}

#[tokio::test]
async fn agent_settings_role_specific_worker_and_reviewer() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    write_agent_settings(
        &ws_agents,
        "worker",
        "Custom worker",
        "Worker instructions.",
        None,
        &["fs.write"],
        true,
    )
    .await;
    write_agent_settings(
        &ws_agents,
        "code-reviewer",
        "Custom reviewer",
        "Reviewer instructions.",
        Some("fast"),
        &["shell", "fs.read"],
        true,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let worker_overrides = executor
        .agent_settings_overrides(AgentRole::Worker)
        .expect("worker strategy must exist");
    assert_eq!(worker_overrides.3, vec!["fs.write"]);

    let reviewer_overrides = executor
        .agent_settings_overrides(AgentRole::Reviewer)
        .expect("reviewer strategy must exist");
    assert_eq!(reviewer_overrides.0.as_deref(), Some("fast"));
    assert_eq!(reviewer_overrides.3, vec!["shell", "fs.read"]);
}

#[tokio::test]
async fn agent_settings_user_only_agent_applied() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    write_agent_settings(
        &usr_agents,
        "default",
        "User-only default",
        "User-only instructions.",
        Some("slow"),
        &[],
        true,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let overrides = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");
    assert_eq!(overrides.0.as_deref(), Some("slow"));
}

#[tokio::test]
async fn agent_settings_no_custom_agents_falls_back_to_builtins() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let planner = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");
    assert_eq!(planner.0, None, "builtin default has no model_profile");
    assert!(planner.2.is_empty(), "builtin default has no skills");

    let worker = executor
        .agent_settings_overrides(AgentRole::Worker)
        .expect("worker strategy must exist");
    assert!(worker.3.is_empty(), "builtin worker has no tool allowlist");

    let reviewer = executor
        .agent_settings_overrides(AgentRole::Reviewer)
        .expect("reviewer strategy must exist");
    assert_eq!(
        reviewer.3,
        vec!["fs.read", "search", "shell"],
        "builtin code-reviewer tools"
    );
}

#[tokio::test]
async fn agent_settings_instructions_override_wired_into_context() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    write_agent_settings(
        &ws_agents,
        "default",
        "Custom planner",
        "Custom system prompt override.",
        None,
        &[],
        true,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let planner = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");

    assert_eq!(planner.0, None);
    assert!(planner.2.is_empty());
}

#[tokio::test]
async fn agent_settings_invalid_agent_not_used() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    tokio::fs::create_dir_all(&ws_agents).await.unwrap();
    tokio::fs::write(
        ws_agents.join("default.md"),
        "This file has no frontmatter; it's invalid.\n",
    )
    .await
    .unwrap();

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;
    assert!(executor.is_available());
}

use super::*;
use std::path::PathBuf;

fn roots(workspace: &std::path::Path, user: &std::path::Path) -> AgentSettingsRoots {
    AgentSettingsRoots {
        workspace_root: Some(workspace.join(".kairox/agents")),
        user_root: Some(user.join(".config/kairox/agents")),
        builtin_root: None,
    }
}

#[test]
fn parses_agent_markdown_with_optional_fields() {
    let raw = r#"---
name: code-reviewer
description: Review code for correctness and missing tests.
tools: ["fs.read", "search"]
model_profile: "fast"
skills: ["kairox-dev-workflow"]
nickname_candidates: ["Reviewer", "Audit"]
enabled: false
---
Review code like an owner.
"#;

    let parsed = parse_agent_markdown(raw).expect("agent frontmatter should parse");

    assert_eq!(parsed.name, "code-reviewer");
    assert_eq!(
        parsed.description,
        "Review code for correctness and missing tests."
    );
    assert_eq!(parsed.tools, vec!["fs.read", "search"]);
    assert_eq!(parsed.model_profile.as_deref(), Some("fast"));
    assert_eq!(parsed.skills, vec!["kairox-dev-workflow"]);
    assert_eq!(parsed.nickname_candidates, vec!["Reviewer", "Audit"]);
    assert!(!parsed.enabled);
    assert_eq!(parsed.instructions, "Review code like an owner.\n");
}

#[test]
fn rejects_invalid_agent_name() {
    let raw = "---\nname: Bad Agent\ndescription: Invalid\n---\nBody\n";
    let error = parse_agent_markdown(raw).expect_err("invalid name should fail");

    assert!(error.to_string().contains("invalid agent name"));
}

#[tokio::test]
async fn discovers_builtin_user_and_project_agents_with_precedence() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    let roots = roots(workspace.path(), user.path());

    write_agent(
        &roots.user_root.clone().unwrap().join("worker.md"),
        "worker",
        "User worker",
        "User worker prompt",
    )
    .await;
    write_agent(
        &roots.workspace_root.clone().unwrap().join("worker.md"),
        "worker",
        "Project worker",
        "Project worker prompt",
    )
    .await;

    let agents = list_agent_settings(roots)
        .await
        .expect("agents should load");
    let worker_defs: Vec<_> = agents
        .iter()
        .filter(|agent| agent.name == "worker")
        .collect();

    assert_eq!(worker_defs.len(), 3);
    let project = worker_defs
        .iter()
        .find(|agent| agent.scope == AgentSettingsScope::Project)
        .expect("project worker");
    let user = worker_defs
        .iter()
        .find(|agent| agent.scope == AgentSettingsScope::User)
        .expect("user worker");
    let builtin = worker_defs
        .iter()
        .find(|agent| agent.scope == AgentSettingsScope::Builtin)
        .expect("builtin worker");

    assert!(project.effective);
    assert!(!user.effective);
    assert_eq!(user.shadowed_by.as_deref(), Some("Project"));
    assert!(!builtin.effective);
    assert_eq!(builtin.shadowed_by.as_deref(), Some("Project"));
}

#[tokio::test]
async fn disabled_project_agent_does_not_shadow_user_agent() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    let roots = roots(workspace.path(), user.path());

    write_agent(
        &roots.user_root.clone().unwrap().join("worker.md"),
        "worker",
        "User worker",
        "User worker prompt",
    )
    .await;
    write_disabled_agent(
        &roots.workspace_root.clone().unwrap().join("worker.md"),
        "worker",
        "Project worker",
        "Project worker prompt",
    )
    .await;

    let agents = list_agent_settings(roots)
        .await
        .expect("agents should load");
    let project = agents
        .iter()
        .find(|agent| agent.name == "worker" && agent.scope == AgentSettingsScope::Project)
        .expect("project worker");
    let user = agents
        .iter()
        .find(|agent| agent.name == "worker" && agent.scope == AgentSettingsScope::User)
        .expect("user worker");

    assert!(!project.effective);
    assert!(user.effective);
}

#[tokio::test]
async fn upsert_writes_agent_markdown_to_target_scope() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    let roots = roots(workspace.path(), user.path());
    let input = AgentSettingsInput {
        scope: AgentSettingsScope::Project,
        name: "test-runner".into(),
        description: "Run focused tests and report failures.".into(),
        tools: vec!["shell".into()],
        model_profile: Some("fast".into()),
        skills: vec![],
        nickname_candidates: vec!["Test".into()],
        enabled: true,
        instructions: "Run tests before claiming success.".into(),
    };

    let view = upsert_agent_settings(roots.clone(), input)
        .await
        .expect("agent should save");
    let path = PathBuf::from(&view.path);
    let raw = tokio::fs::read_to_string(&path)
        .await
        .expect("agent file should exist");

    assert_eq!(view.scope, AgentSettingsScope::Project);
    assert!(raw.contains("name: test-runner"));
    assert!(raw.contains("Run tests before claiming success."));
    assert!(path.starts_with(roots.workspace_root.unwrap()));
}

#[tokio::test]
async fn deleting_builtin_agent_is_rejected() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    let roots = roots(workspace.path(), user.path());

    let error = delete_agent_settings(roots, "Builtin:worker")
        .await
        .expect_err("builtin deletion should fail");

    assert!(error.to_string().contains("cannot delete built-in agent"));
}

async fn write_agent(path: &std::path::Path, name: &str, description: &str, body: &str) {
    let parent = path.parent().expect("agent path should have parent");
    tokio::fs::create_dir_all(parent)
        .await
        .expect("agent dir should be created");
    tokio::fs::write(
        path,
        format!("---\nname: {name}\ndescription: {description}\n---\n{body}\n"),
    )
    .await
    .expect("agent should be written");
}

async fn write_disabled_agent(path: &std::path::Path, name: &str, description: &str, body: &str) {
    let parent = path.parent().expect("agent path should have parent");
    tokio::fs::create_dir_all(parent)
        .await
        .expect("agent dir should be created");
    tokio::fs::write(
        path,
        format!("---\nname: {name}\ndescription: {description}\nenabled: false\n---\n{body}\n"),
    )
    .await
    .expect("agent should be written");
}

use std::path::{Path, PathBuf};

use agent_core::facade::{AgentSettingsInput, AgentSettingsScope, AgentSettingsView};
use agent_core::{CoreError, Result};

mod parser;
mod projection;

pub use parser::{parse_agent_markdown, ParsedAgentMarkdown};
pub(crate) use projection::effective_agent_by_name;

use parser::{render_agent_markdown, validate_agent_name};

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct AgentSettingsRoots {
    pub workspace_root: Option<PathBuf>,
    pub user_root: Option<PathBuf>,
    pub builtin_root: Option<PathBuf>,
}

pub fn build_default_agent_settings_roots(home: &Path, workspace: &Path) -> AgentSettingsRoots {
    AgentSettingsRoots {
        workspace_root: Some(workspace.join(".kairox/agents")),
        user_root: Some(home.join(".config/kairox/agents")),
        builtin_root: None,
    }
}

pub async fn list_agent_settings(roots: AgentSettingsRoots) -> Result<Vec<AgentSettingsView>> {
    projection::list_agent_settings(&roots).await
}

pub async fn upsert_agent_settings(
    roots: AgentSettingsRoots,
    input: AgentSettingsInput,
) -> Result<AgentSettingsView> {
    if input.scope == AgentSettingsScope::Builtin {
        return Err(CoreError::InvalidState(
            "cannot modify built-in agent".to_string(),
        ));
    }
    validate_agent_name(&input.name)?;
    let root = root_for_scope(&roots, input.scope).ok_or_else(|| {
        CoreError::InvalidState(format!(
            "agent root not configured for {}",
            scope_label(input.scope)
        ))
    })?;
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to create agent dir: {error}")))?;
    let path = root.join(format!("{}.md", input.name));
    let raw = render_agent_markdown(&input)?;
    tokio::fs::write(&path, raw)
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to write agent: {error}")))?;

    projection::find_agent_settings_view(&roots, &settings_id(input.scope, &input.name)).await
}

pub async fn delete_agent_settings(roots: AgentSettingsRoots, agent_id: &str) -> Result<()> {
    let view = projection::find_agent_settings_view(&roots, agent_id).await?;
    if view.scope == AgentSettingsScope::Builtin {
        return Err(CoreError::InvalidState(format!(
            "cannot delete built-in agent: {}",
            view.name
        )));
    }
    let root = root_for_scope(&roots, view.scope).ok_or_else(|| {
        CoreError::InvalidState(format!(
            "agent root not configured for {}",
            scope_label(view.scope)
        ))
    })?;
    let path = PathBuf::from(&view.path);
    validate_file_under_root(&path, &root)?;
    tokio::fs::remove_file(&path)
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to delete agent: {error}")))
}

pub async fn copy_agent_settings(
    roots: AgentSettingsRoots,
    agent_id: &str,
    scope: AgentSettingsScope,
) -> Result<AgentSettingsView> {
    if scope == AgentSettingsScope::Builtin {
        return Err(CoreError::InvalidState(
            "cannot copy agent to built-in scope".into(),
        ));
    }
    let view = projection::find_agent_settings_view(&roots, agent_id).await?;
    upsert_agent_settings(
        roots,
        AgentSettingsInput {
            scope,
            name: view.name,
            description: view.description,
            tools: view.tools,
            model_profile: view.model_profile,
            permission_mode: view.permission_mode,
            skills: view.skills,
            nickname_candidates: view.nickname_candidates,
            enabled: view.enabled,
            instructions: view.instructions,
        },
    )
    .await
}

fn validate_file_under_root(path: &Path, root: &Path) -> Result<()> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(CoreError::InvalidState(format!(
            "agent path escapes root: {}",
            path.display()
        )))
    }
}

fn root_for_scope(roots: &AgentSettingsRoots, scope: AgentSettingsScope) -> Option<PathBuf> {
    match scope {
        AgentSettingsScope::Builtin => roots.builtin_root.clone(),
        AgentSettingsScope::User => roots.user_root.clone(),
        AgentSettingsScope::Project => roots.workspace_root.clone(),
    }
}

fn settings_id(scope: AgentSettingsScope, name: &str) -> String {
    format!("{}:{name}", scope_label(scope))
}

fn scope_label(scope: AgentSettingsScope) -> &'static str {
    match scope {
        AgentSettingsScope::Builtin => "Builtin",
        AgentSettingsScope::User => "User",
        AgentSettingsScope::Project => "Project",
    }
}

#[cfg(test)]
mod tests {
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
permission_mode: "read_only"
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
        assert_eq!(parsed.permission_mode.as_deref(), Some("read_only"));
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
            permission_mode: Some("workspace_write".into()),
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

    async fn write_disabled_agent(
        path: &std::path::Path,
        name: &str,
        description: &str,
        body: &str,
    ) {
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
}

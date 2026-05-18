use std::collections::HashMap;
use std::path::{Path, PathBuf};

use agent_core::facade::{AgentSettingsInput, AgentSettingsScope, AgentSettingsView};
use agent_core::{CoreError, Result};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParsedAgentMarkdown {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub model_profile: Option<String>,
    pub permission_mode: Option<String>,
    pub skills: Vec<String>,
    pub nickname_candidates: Vec<String>,
    pub enabled: bool,
    pub instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawAgentFrontmatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    model_profile: Option<String>,
    #[serde(default)]
    permission_mode: Option<String>,
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    nickname_candidates: Vec<String>,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

pub fn parse_agent_markdown(raw: &str) -> Result<ParsedAgentMarkdown> {
    let frontmatter_block = raw
        .strip_prefix("---\n")
        .ok_or_else(|| CoreError::InvalidState("missing agent frontmatter".into()))?;
    let (frontmatter_yaml, instructions) = frontmatter_block
        .split_once("\n---\n")
        .ok_or_else(|| CoreError::InvalidState("missing agent frontmatter".into()))?;

    let frontmatter: RawAgentFrontmatter = serde_yaml::from_str(frontmatter_yaml)
        .map_err(|error| CoreError::InvalidState(format!("invalid agent frontmatter: {error}")))?;
    let name = frontmatter
        .name
        .ok_or_else(|| CoreError::InvalidState("missing required agent field: name".into()))?;
    validate_agent_name(&name)?;
    let description = frontmatter.description.ok_or_else(|| {
        CoreError::InvalidState("missing required agent field: description".into())
    })?;

    Ok(ParsedAgentMarkdown {
        name,
        description,
        tools: frontmatter.tools,
        model_profile: frontmatter.model_profile,
        permission_mode: frontmatter.permission_mode,
        skills: frontmatter.skills,
        nickname_candidates: frontmatter.nickname_candidates,
        enabled: frontmatter.enabled,
        instructions: instructions.to_string(),
    })
}

pub async fn list_agent_settings(roots: AgentSettingsRoots) -> Result<Vec<AgentSettingsView>> {
    let mut views = builtin_agent_views(&roots);
    if let Some(root) = &roots.user_root {
        views.extend(discover_agent_files(root, AgentSettingsScope::User).await?);
    }
    if let Some(root) = &roots.workspace_root {
        views.extend(discover_agent_files(root, AgentSettingsScope::Project).await?);
    }
    mark_effective_agents(&mut views);
    views.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| right.scope.cmp(&left.scope))
    });
    Ok(views)
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

    find_agent_settings_view(roots, &settings_id(input.scope, &input.name)).await
}

pub async fn delete_agent_settings(roots: AgentSettingsRoots, agent_id: &str) -> Result<()> {
    let view = find_agent_settings_view(roots.clone(), agent_id).await?;
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
    let view = find_agent_settings_view(roots.clone(), agent_id).await?;
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

async fn find_agent_settings_view(
    roots: AgentSettingsRoots,
    agent_identifier: &str,
) -> Result<AgentSettingsView> {
    let views = list_agent_settings(roots).await?;
    let matching_settings_id_views = views
        .iter()
        .filter(|view| view.settings_id == agent_identifier)
        .cloned()
        .collect::<Vec<_>>();
    match matching_settings_id_views.as_slice() {
        [view] => return Ok(view.clone()),
        [] => {}
        _ => {
            return Err(CoreError::InvalidState(format!(
                "ambiguous agent settings id: {agent_identifier}"
            )));
        }
    }

    let matching_views = views
        .into_iter()
        .filter(|view| view.name == agent_identifier)
        .collect::<Vec<_>>();
    match matching_views.as_slice() {
        [view] => Ok(view.clone()),
        [] => Err(CoreError::InvalidState(format!(
            "agent not found: {agent_identifier}"
        ))),
        views => Err(CoreError::InvalidState(format!(
            "ambiguous agent name: {agent_identifier}; matching scopes: {}",
            views
                .iter()
                .map(|view| scope_label(view.scope))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

async fn discover_agent_files(
    root: &Path,
    scope: AgentSettingsScope,
) -> Result<Vec<AgentSettingsView>> {
    let mut views = Vec::new();
    let mut entries = match tokio::fs::read_dir(root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(CoreError::InvalidState(format!(
                "failed to read agent dir: {error}"
            )));
        }
    };

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to read agent entry: {error}")))?
    {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let raw = tokio::fs::read_to_string(&path)
            .await
            .map_err(|error| CoreError::InvalidState(format!("failed to read agent: {error}")))?;
        match parse_agent_markdown(&raw) {
            Ok(agent) => views.push(view_from_parsed(scope, path, agent, true, None)),
            Err(error) => views.push(invalid_view(scope, path, error.to_string())),
        }
    }

    Ok(views)
}

fn builtin_agent_views(roots: &AgentSettingsRoots) -> Vec<AgentSettingsView> {
    let builtin_root = roots.builtin_root.clone();
    builtin_agents()
        .into_iter()
        .map(|agent| {
            let path = builtin_root
                .as_ref()
                .map(|root| root.join(format!("{}.md", agent.name)))
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| format!("builtin://{}", agent.name));
            AgentSettingsView {
                settings_id: settings_id(AgentSettingsScope::Builtin, &agent.name),
                name: agent.name,
                description: agent.description,
                scope: AgentSettingsScope::Builtin,
                path,
                tools: agent.tools,
                model_profile: agent.model_profile,
                permission_mode: agent.permission_mode,
                skills: agent.skills,
                nickname_candidates: agent.nickname_candidates,
                enabled: agent.enabled,
                instructions: agent.instructions,
                effective: false,
                shadowed_by: None,
                valid: true,
                validation_error: None,
                editable: false,
                deletable: false,
            }
        })
        .collect()
}

fn builtin_agents() -> Vec<ParsedAgentMarkdown> {
    vec![
        ParsedAgentMarkdown {
            name: "default".into(),
            description: "General-purpose fallback agent.".into(),
            tools: Vec::new(),
            model_profile: None,
            permission_mode: None,
            skills: Vec::new(),
            nickname_candidates: vec!["Default".into()],
            enabled: true,
            instructions: "Handle general tasks that do not need a more specific agent.".into(),
        },
        ParsedAgentMarkdown {
            name: "worker".into(),
            description: "Execution-focused agent for implementation and fixes.".into(),
            tools: Vec::new(),
            model_profile: None,
            permission_mode: Some("workspace_write".into()),
            skills: Vec::new(),
            nickname_candidates: vec!["Worker".into()],
            enabled: true,
            instructions:
                "Implement scoped changes, run focused validation, and report changed files."
                    .into(),
        },
        ParsedAgentMarkdown {
            name: "explorer".into(),
            description: "Read-heavy codebase exploration agent.".into(),
            tools: vec!["fs.read".into(), "search".into()],
            model_profile: None,
            permission_mode: Some("read_only".into()),
            skills: Vec::new(),
            nickname_candidates: vec!["Explorer".into()],
            enabled: true,
            instructions: "Map code paths, cite files and symbols, and avoid editing files.".into(),
        },
        ParsedAgentMarkdown {
            name: "code-reviewer".into(),
            description: "Review code for correctness, security, regressions, and missing tests."
                .into(),
            tools: vec!["fs.read".into(), "search".into(), "shell".into()],
            model_profile: None,
            permission_mode: Some("read_only".into()),
            skills: Vec::new(),
            nickname_candidates: vec!["Reviewer".into()],
            enabled: true,
            instructions: "Lead with concrete findings ordered by severity. Focus on bugs, regressions, security, and missing tests.".into(),
        },
        ParsedAgentMarkdown {
            name: "test-runner".into(),
            description: "Run focused tests, diagnose failures, and report validation evidence."
                .into(),
            tools: vec!["shell".into(), "fs.read".into(), "search".into()],
            model_profile: None,
            permission_mode: Some("workspace_write".into()),
            skills: Vec::new(),
            nickname_candidates: vec!["Test".into()],
            enabled: true,
            instructions: "Run the smallest useful test first, inspect failures, and preserve test intent when fixing issues.".into(),
        },
    ]
}

fn mark_effective_agents(views: &mut [AgentSettingsView]) {
    let mut highest_by_name: HashMap<String, AgentSettingsScope> = HashMap::new();
    for view in views.iter().filter(|view| view.valid && view.enabled) {
        highest_by_name
            .entry(view.name.clone())
            .and_modify(|scope| {
                if view.scope > *scope {
                    *scope = view.scope;
                }
            })
            .or_insert(view.scope);
    }

    for view in views {
        let Some(active_scope) = highest_by_name.get(&view.name) else {
            continue;
        };
        view.effective = view.valid && view.scope == *active_scope;
        view.shadowed_by = if view.effective {
            None
        } else {
            Some(scope_label(*active_scope).to_string())
        };
    }
}

fn view_from_parsed(
    scope: AgentSettingsScope,
    path: PathBuf,
    agent: ParsedAgentMarkdown,
    valid: bool,
    validation_error: Option<String>,
) -> AgentSettingsView {
    AgentSettingsView {
        settings_id: settings_id(scope, &agent.name),
        name: agent.name,
        description: agent.description,
        scope,
        path: path.display().to_string(),
        tools: agent.tools,
        model_profile: agent.model_profile,
        permission_mode: agent.permission_mode,
        skills: agent.skills,
        nickname_candidates: agent.nickname_candidates,
        enabled: agent.enabled,
        instructions: agent.instructions,
        effective: false,
        shadowed_by: None,
        valid,
        validation_error,
        editable: scope != AgentSettingsScope::Builtin,
        deletable: scope != AgentSettingsScope::Builtin,
    }
}

fn invalid_view(
    scope: AgentSettingsScope,
    path: PathBuf,
    validation_error: String,
) -> AgentSettingsView {
    let name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("invalid-agent")
        .to_string();
    AgentSettingsView {
        settings_id: settings_id(scope, &name),
        name,
        description: String::new(),
        scope,
        path: path.display().to_string(),
        tools: Vec::new(),
        model_profile: None,
        permission_mode: None,
        skills: Vec::new(),
        nickname_candidates: Vec::new(),
        enabled: false,
        instructions: String::new(),
        effective: false,
        shadowed_by: None,
        valid: false,
        validation_error: Some(validation_error),
        editable: scope != AgentSettingsScope::Builtin,
        deletable: scope != AgentSettingsScope::Builtin,
    }
}

fn render_agent_markdown(input: &AgentSettingsInput) -> Result<String> {
    let frontmatter = RawAgentFrontmatter {
        name: Some(input.name.clone()),
        description: Some(input.description.clone()),
        tools: input.tools.clone(),
        model_profile: input.model_profile.clone(),
        permission_mode: input.permission_mode.clone(),
        skills: input.skills.clone(),
        nickname_candidates: input.nickname_candidates.clone(),
        enabled: input.enabled,
    };
    let mut yaml = serde_yaml::to_string(&frontmatter)
        .map_err(|error| CoreError::InvalidState(format!("failed to render agent: {error}")))?;
    if let Some(stripped) = yaml.strip_prefix("---\n") {
        yaml = stripped.to_string();
    }
    Ok(format!(
        "---\n{}---\n{}\n",
        yaml,
        input.instructions.trim_end()
    ))
}

fn validate_agent_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(CoreError::InvalidState("invalid agent name: empty".into()));
    };
    if !first.is_ascii_lowercase() {
        return Err(CoreError::InvalidState(format!(
            "invalid agent name: {name}"
        )));
    }
    if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_') {
        return Err(CoreError::InvalidState(format!(
            "invalid agent name: {name}"
        )));
    }
    Ok(())
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

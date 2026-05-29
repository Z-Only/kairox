use std::collections::HashMap;
use std::path::{Path, PathBuf};

use agent_core::facade::{AgentSettingsScope, AgentSettingsView};
use agent_core::{CoreError, Result};

use super::parser::{parse_agent_markdown, ParsedAgentMarkdown};
use super::{scope_label, settings_id, AgentSettingsRoots};

pub(super) async fn list_agent_settings(
    roots: &AgentSettingsRoots,
) -> Result<Vec<AgentSettingsView>> {
    let mut views = builtin_agent_views(roots);
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

pub(super) async fn find_agent_settings_view(
    roots: &AgentSettingsRoots,
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

/// Find the effective (highest-precedence, enabled) agent for a given name.
pub(crate) fn effective_agent_by_name<'a>(
    views: &'a [AgentSettingsView],
    name: &str,
) -> Option<&'a AgentSettingsView> {
    views
        .iter()
        .find(|view| view.effective && view.valid && view.name == name)
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

#[cfg(test)]
#[path = "projection_tests.rs"]
mod tests;

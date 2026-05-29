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
#[path = "agent_settings_tests.rs"]
mod tests;

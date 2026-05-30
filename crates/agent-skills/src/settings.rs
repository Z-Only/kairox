use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::registry::SkillRoot;
use crate::state::{read_skills_state, SkillsStateFile};
use crate::types::{SkillActivationMode, SkillId, SkillSourceKind};
use crate::{parse_skill_markdown, Result};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SkillSettingsProjection {
    pub skills: Vec<LocalSkillSettingsView>,
    pub state_errors: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LocalSkillSettingsView {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub scope: SkillSourceKind,
    pub path: PathBuf,
    pub enabled: bool,
    pub activation_mode: SkillActivationMode,
    pub tools: Vec<String>,
    pub can_request_tools: Vec<String>,
    pub install_source: String,
    pub update_available: Option<bool>,
    pub effective: bool,
    pub shadowed_by: Option<String>,
    pub valid: bool,
    pub validation_error: Option<String>,
}

pub async fn discover_skill_settings(roots: Vec<SkillRoot>) -> Result<SkillSettingsProjection> {
    let mut skills = Vec::new();
    let mut state_errors = Vec::new();

    for root in roots {
        if !tokio::fs::try_exists(&root.path).await? {
            continue;
        }

        let state_path = root.path.join("skills-state.toml");
        let state = match read_skills_state(&state_path).await {
            Ok(state) => state,
            Err(error) => {
                state_errors.push(format!("{}: {error}", state_path.display()));
                SkillsStateFile::default()
            }
        };

        let mut child_entries = tokio::fs::read_dir(&root.path).await?;
        while let Some(child_entry) = child_entries.next_entry().await? {
            if !child_entry.file_type().await?.is_dir() {
                continue;
            }

            let skill_path = child_entry.path().join("SKILL.md");
            if !tokio::fs::try_exists(&skill_path).await? {
                continue;
            }

            let view = read_local_skill_settings_view(root.kind, skill_path, &state).await;
            skills.push(view);
        }
    }

    apply_effective_skill_markers(&mut skills);
    sort_skill_settings(&mut skills);

    Ok(SkillSettingsProjection {
        skills,
        state_errors,
    })
}

async fn read_local_skill_settings_view(
    scope: SkillSourceKind,
    skill_path: PathBuf,
    state: &SkillsStateFile,
) -> LocalSkillSettingsView {
    let raw_skill_markdown = match tokio::fs::read_to_string(&skill_path).await {
        Ok(raw_skill_markdown) => raw_skill_markdown,
        Err(error) => {
            let fallback_id = fallback_skill_id(&skill_path);
            return invalid_skill_settings_view(
                fallback_id,
                scope,
                skill_path,
                state,
                error.to_string(),
            );
        }
    };

    let parsed_skill = match parse_skill_markdown(&raw_skill_markdown) {
        Ok(parsed_skill) => parsed_skill,
        Err(error) => {
            let fallback_id = extract_frontmatter_name(&raw_skill_markdown)
                .unwrap_or_else(|| fallback_skill_id(&skill_path));
            return invalid_skill_settings_view(
                fallback_id,
                scope,
                skill_path,
                state,
                error.to_string(),
            );
        }
    };

    let skill_id = parsed_skill.frontmatter.name;
    let state_entry = state.skill(&skill_id);
    let activation_mode = state_entry
        .and_then(|entry| entry.activation_mode)
        .unwrap_or(parsed_skill.activation.mode);

    LocalSkillSettingsView {
        id: SkillId::new(skill_id.clone()),
        name: skill_id.clone(),
        description: parsed_skill.frontmatter.description,
        version: parsed_skill.frontmatter.version,
        scope,
        path: skill_path,
        enabled: state_entry.and_then(|entry| entry.enabled).unwrap_or(true),
        activation_mode,
        tools: parsed_skill.permissions.tools,
        can_request_tools: parsed_skill.permissions.can_request_tools,
        install_source: state_entry
            .and_then(|entry| entry.install_source.clone())
            .unwrap_or_else(|| default_install_source(scope).to_owned()),
        update_available: state_entry.and_then(|entry| entry.update_available),
        effective: false,
        shadowed_by: None,
        valid: true,
        validation_error: None,
    }
}

fn invalid_skill_settings_view(
    skill_id: String,
    scope: SkillSourceKind,
    skill_path: PathBuf,
    state: &SkillsStateFile,
    validation_error: String,
) -> LocalSkillSettingsView {
    let state_entry = state.skill(&skill_id);

    LocalSkillSettingsView {
        id: SkillId::new(skill_id.clone()),
        name: skill_id,
        description: String::new(),
        version: state_entry.and_then(|entry| entry.version.clone()),
        scope,
        path: skill_path,
        enabled: state_entry.and_then(|entry| entry.enabled).unwrap_or(true),
        activation_mode: state_entry
            .and_then(|entry| entry.activation_mode)
            .unwrap_or_default(),
        tools: Vec::new(),
        can_request_tools: Vec::new(),
        install_source: state_entry
            .and_then(|entry| entry.install_source.clone())
            .unwrap_or_else(|| default_install_source(scope).to_owned()),
        update_available: state_entry.and_then(|entry| entry.update_available),
        effective: false,
        shadowed_by: None,
        valid: false,
        validation_error: Some(validation_error),
    }
}

fn apply_effective_skill_markers(skills: &mut [LocalSkillSettingsView]) {
    let mut effective_scope_by_skill_id = BTreeMap::new();

    for skill in skills.iter() {
        let existing_scope = effective_scope_by_skill_id
            .get(skill.id.as_str())
            .copied()
            .unwrap_or(skill.scope);
        if scope_priority(skill.scope) >= scope_priority(existing_scope) {
            effective_scope_by_skill_id.insert(skill.id.as_str().to_owned(), skill.scope);
        }
    }

    for skill in skills.iter_mut() {
        let effective_scope = effective_scope_by_skill_id
            .get(skill.id.as_str())
            .copied()
            .unwrap_or(skill.scope);
        skill.effective = skill.scope == effective_scope;
        skill.shadowed_by = if skill.effective {
            None
        } else {
            Some(scope_label(effective_scope).to_owned())
        };
    }
}

fn scope_priority(scope: SkillSourceKind) -> u8 {
    match scope {
        SkillSourceKind::Builtin => 0,
        SkillSourceKind::User => 1,
        SkillSourceKind::Workspace => 2,
        SkillSourceKind::Plugin => 3,
    }
}

fn scope_label(scope: SkillSourceKind) -> &'static str {
    match scope {
        SkillSourceKind::Builtin => "builtin",
        SkillSourceKind::User => "user",
        SkillSourceKind::Workspace => "workspace",
        SkillSourceKind::Plugin => "plugin",
    }
}

fn sort_skill_settings(skills: &mut [LocalSkillSettingsView]) {
    skills.sort_by(|left, right| {
        left.id
            .as_str()
            .cmp(right.id.as_str())
            .then_with(|| scope_priority(right.scope).cmp(&scope_priority(left.scope)))
            .then_with(|| left.path.cmp(&right.path))
    });
}

fn default_install_source(scope: SkillSourceKind) -> &'static str {
    match scope {
        SkillSourceKind::Builtin => "builtin",
        SkillSourceKind::User | SkillSourceKind::Workspace => "local",
        SkillSourceKind::Plugin => "plugin",
    }
}

fn fallback_skill_id(skill_path: &Path) -> String {
    skill_path
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_owned()
}

fn extract_frontmatter_name(raw_skill_markdown: &str) -> Option<String> {
    let frontmatter_block = raw_skill_markdown.strip_prefix("---\n")?;
    let (frontmatter_yaml, _) = frontmatter_block.split_once("\n---\n")?;

    for line in frontmatter_yaml.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.trim() == "name" {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }

    None
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_core::{ActiveSkillView, DomainEvent, EventPayload, SkillDetail, SkillView};

use crate::plugin_settings::PluginSettingsRoots;
use crate::skill_settings::SkillSettingsRoots;
use agent_skills::{SkillActivationMode, SkillDocument, SkillMetadata, SkillRoot, SkillSourceKind};

const BUILTIN_SKILLS_DIR_NAME: &str = "builtin-skills";

struct BuiltinSkillAsset {
    directory_name: &'static str,
    markdown: &'static str,
}

const BUILTIN_SKILL_ASSETS: &[BuiltinSkillAsset] = &[BuiltinSkillAsset {
    directory_name: "skill-creator",
    markdown: include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/builtin-skills/skill-creator/SKILL.md"
    )),
}];

pub fn builtin_skills_root(data_dir: &Path) -> PathBuf {
    data_dir.join(BUILTIN_SKILLS_DIR_NAME)
}

pub async fn ensure_builtin_skills_root(data_dir: &Path) -> crate::Result<PathBuf> {
    let root = builtin_skills_root(data_dir);
    for asset in BUILTIN_SKILL_ASSETS {
        let skill_directory = root.join(asset.directory_name);
        tokio::fs::create_dir_all(&skill_directory)
            .await
            .map_err(|error| {
                crate::RuntimeError::Other(format!(
                    "create builtin skill dir {}: {error}",
                    skill_directory.display()
                ))
            })?;
        let skill_path = skill_directory.join("SKILL.md");
        tokio::fs::write(&skill_path, asset.markdown)
            .await
            .map_err(|error| {
                crate::RuntimeError::Other(format!(
                    "write builtin skill {}: {error}",
                    skill_path.display()
                ))
            })?;
    }
    Ok(root)
}

pub fn build_default_skill_roots(home: &Path, workspace: &Path) -> Vec<SkillRoot> {
    vec![
        SkillRoot::new(
            SkillSourceKind::Builtin,
            builtin_skills_root(&home.join(".kairox")),
        ),
        SkillRoot::new(SkillSourceKind::User, home.join(".config/kairox/skills")),
        SkillRoot::new(SkillSourceKind::Workspace, workspace.join(".kairox/skills")),
    ]
}

pub fn build_default_skill_settings_roots(home: &Path, workspace: &Path) -> SkillSettingsRoots {
    SkillSettingsRoots {
        workspace_root: Some(workspace.join(".kairox/skills")),
        user_root: Some(home.join(".config/kairox/skills")),
        builtin_root: Some(builtin_skills_root(&home.join(".kairox"))),
        plugin_roots: Vec::new(),
    }
}

pub(crate) fn skill_settings_roots_for_project_root(
    mut roots: SkillSettingsRoots,
    project_root: &Path,
) -> SkillSettingsRoots {
    roots.workspace_root = Some(project_root.join(".kairox/skills"));
    roots
}

pub(crate) async fn discover_skill_registry_for_settings_roots(
    roots: SkillSettingsRoots,
    fallback: Option<Arc<dyn agent_skills::SkillRegistry>>,
) -> agent_core::Result<Option<Arc<dyn agent_skills::SkillRegistry>>> {
    let skill_roots = crate::skill_settings::skill_roots(&roots);
    if skill_roots.is_empty() {
        return Ok(fallback);
    }

    let registry = agent_skills::FileSkillRegistry::discover(skill_roots)
        .await
        .map_err(|error| {
            agent_core::CoreError::InvalidState(format!("skill discovery: {error}"))
        })?;
    Ok(Some(Arc::new(registry)))
}

pub async fn build_plugin_skill_roots(
    plugin_settings_roots: &PluginSettingsRoots,
) -> Vec<SkillRoot> {
    let plugin_roots = crate::plugin_settings::plugin_roots(plugin_settings_roots);
    let Ok(projection) = agent_plugins::discover_plugin_settings(plugin_roots).await else {
        return vec![];
    };
    projection
        .plugins
        .into_iter()
        .filter(|p| p.enabled && p.valid && p.manifest.inventory.skill_count > 0)
        .map(|p| {
            let skill_dir = p.manifest.plugin_root.join("skills");
            SkillRoot::with_namespace(SkillSourceKind::Plugin, skill_dir, p.manifest.name.clone())
        })
        .collect()
}

pub fn render_active_skill_block(name: &str, source: &str, body_markdown: &str) -> String {
    format!(
        "<skill name=\"{}\" source=\"{}\">\n{}\n</skill>",
        name, source, body_markdown
    )
}

pub(crate) fn active_skill_ids_from_events(events: &[DomainEvent]) -> Vec<String> {
    let mut skill_ids = Vec::new();
    for event in events {
        match &event.payload {
            EventPayload::SkillActivated { skill_id, .. }
                if !skill_ids.iter().any(|existing| existing == skill_id) =>
            {
                skill_ids.push(skill_id.clone());
            }
            EventPayload::SkillDeactivated { skill_id, .. } => {
                skill_ids.retain(|existing| existing != skill_id);
            }
            _ => {}
        }
    }
    skill_ids
}

pub fn skill_metadata_to_view(metadata: &SkillMetadata) -> SkillView {
    SkillView {
        id: metadata.id.as_str().to_string(),
        name: metadata.name.clone(),
        description: metadata.description.clone(),
        version: metadata.version.clone(),
        source: skill_source_kind_to_string(metadata.source.kind),
        activation_mode: skill_activation_mode_to_string(metadata.activation.mode),
        keywords: metadata.activation.keywords.clone(),
        tools: metadata.permissions.tools.clone(),
        can_request_tools: metadata.permissions.can_request_tools.clone(),
        valid: true,
        validation_error: None,
    }
}

pub fn skill_document_to_detail(document: SkillDocument) -> SkillDetail {
    SkillDetail {
        view: skill_metadata_to_view(&document.metadata),
        body_markdown: document.body_markdown,
    }
}

pub fn skill_metadata_to_active_view(metadata: &SkillMetadata) -> ActiveSkillView {
    ActiveSkillView {
        skill_id: metadata.id.as_str().to_string(),
        name: metadata.name.clone(),
        source: skill_source_kind_to_string(metadata.source.kind),
        activation_mode: skill_activation_mode_to_string(metadata.activation.mode),
    }
}

pub fn skill_source_kind_to_string(kind: SkillSourceKind) -> String {
    match kind {
        SkillSourceKind::Builtin => "builtin",
        SkillSourceKind::User => "user",
        SkillSourceKind::Workspace => "workspace",
        SkillSourceKind::Plugin => "plugin",
    }
    .to_string()
}

pub fn skill_activation_mode_to_string(mode: SkillActivationMode) -> String {
    match mode {
        SkillActivationMode::Manual => "manual",
        SkillActivationMode::Suggest => "suggest",
        SkillActivationMode::Auto => "auto",
    }
    .to_string()
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;

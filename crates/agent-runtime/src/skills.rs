use std::path::Path;

use agent_core::{ActiveSkillView, SkillDetail, SkillView};

use crate::plugin_settings::PluginSettingsRoots;
use crate::skill_settings::SkillSettingsRoots;
use agent_skills::{SkillActivationMode, SkillDocument, SkillMetadata, SkillRoot, SkillSourceKind};

pub fn build_default_skill_roots(home: &Path, workspace: &Path) -> Vec<SkillRoot> {
    vec![
        SkillRoot::new(SkillSourceKind::User, home.join(".config/kairox/skills")),
        SkillRoot::new(SkillSourceKind::Workspace, workspace.join(".kairox/skills")),
    ]
}

pub fn build_default_skill_settings_roots(home: &Path, workspace: &Path) -> SkillSettingsRoots {
    SkillSettingsRoots {
        workspace_root: Some(workspace.join(".kairox/skills")),
        user_root: Some(home.join(".config/kairox/skills")),
        builtin_root: None,
        plugin_roots: Vec::new(),
    }
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

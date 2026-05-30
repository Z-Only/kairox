//! Skills and skill settings DTOs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub source: String,
    pub activation_mode: String,
    pub keywords: Vec<String>,
    pub tools: Vec<String>,
    pub can_request_tools: Vec<String>,
    pub valid: bool,
    pub validation_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillDetail {
    pub view: SkillView,
    pub body_markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ActivateSkillRequest {
    pub workspace_id: crate::WorkspaceId,
    pub session_id: crate::SessionId,
    pub skill_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DeactivateSkillRequest {
    pub workspace_id: crate::WorkspaceId,
    pub session_id: crate::SessionId,
    pub skill_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ActiveSkillView {
    pub skill_id: String,
    pub name: String,
    pub source: String,
    pub activation_mode: String,
}

/// Concrete effective-view wrapper for skill settings.
/// Combines [`EffectiveItem`] metadata with a [`SkillSettingsView`].
/// This is a non-generic type so it can safely derive both serde and specta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EffectiveSkillView {
    pub value: SkillSettingsView,
    pub source: crate::config_scope::ConfigScope,
    pub overrides: Option<crate::config_scope::ConfigScope>,
    pub enabled: bool,
    #[serde(rename = "disabledBy")]
    pub disabled_by: Option<crate::config_scope::ConfigScope>,
    pub writable: bool,
    pub deletable: bool,
}

impl EffectiveSkillView {
    pub fn from_skill_settings(view: SkillSettingsView) -> Self {
        let source = match view.scope {
            SkillSettingsScope::Project => crate::config_scope::ConfigScope::Project,
            SkillSettingsScope::User => crate::config_scope::ConfigScope::User,
            SkillSettingsScope::Builtin => crate::config_scope::ConfigScope::Builtin,
            SkillSettingsScope::Plugin => crate::config_scope::ConfigScope::Local,
        };
        let effectively_enabled = view.effective && view.enabled;
        Self {
            source,
            overrides: if view.effective && source > crate::config_scope::ConfigScope::Builtin {
                Some(source)
            } else {
                None
            },
            enabled: effectively_enabled,
            disabled_by: if view.effective { None } else { Some(source) },
            writable: view.editable,
            deletable: view.deletable,
            value: view,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SkillSettingsScope {
    Project,
    User,
    Builtin,
    Plugin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SkillInstallSource {
    Local,
    Registry,
    Github,
    Builtin,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SkillUpdateState {
    Unknown,
    UpToDate,
    UpdateAvailable,
    CheckFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillSettingsView {
    pub settings_id: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub scope: SkillSettingsScope,
    pub path: String,
    pub enabled: bool,
    pub activation_mode: String,
    pub install_source: SkillInstallSource,
    pub update_state: SkillUpdateState,
    pub effective: bool,
    pub shadowed_by: Option<String>,
    pub valid: bool,
    pub validation_error: Option<String>,
    pub editable: bool,
    pub deletable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillSettingsDetail {
    pub view: SkillSettingsView,
    pub content: String,
    pub source_chain: Vec<SkillSettingsView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RemoteSkillSearchResult {
    pub name: String,
    pub description: String,
    pub repository: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub install_count: Option<u64>,
    pub source_url: String,
    pub package: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SkillInstallTarget {
    Project,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstallRemoteSkillRequest {
    pub package: String,
    pub source: String,
    pub target: SkillInstallTarget,
    pub package_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstallGithubSkillRequest {
    pub source: String,
    pub target: SkillInstallTarget,
}

/// A single skill entry returned by the skills catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillCatalogEntry {
    pub catalog_id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub source_url: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub install_count: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub github_stars: Option<u64>,
    pub security_score: Option<u32>,
    pub rating: Option<f64>,
    pub package: String,
    pub package_url: Option<String>,
}

/// Query against the skills catalog.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillCatalogQuery {
    pub keyword: Option<String>,
    pub sources: Option<Vec<String>>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub limit: Option<usize>,
}

/// JSON field mapping for a skill source API response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillFieldMappingView {
    pub name_path: String,
    pub description_path: String,
    pub install_count_path: Option<String>,
    pub github_stars_path: Option<String>,
    pub package_path: String,
    pub source_url_path: Option<String>,
}

impl Default for SkillFieldMappingView {
    fn default() -> Self {
        Self {
            name_path: "name".into(),
            description_path: "description".into(),
            install_count_path: Some("installs".into()),
            github_stars_path: None,
            package_path: "id".into(),
            source_url_path: None,
        }
    }
}

/// A configured skill catalog source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillSourceView {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub url: String,
    pub search_template: String,
    pub download_template: String,
    pub list_template: Option<String>,
    pub detail_template: Option<String>,
    pub field_mapping: SkillFieldMappingView,
    pub enabled: bool,
    pub priority: u32,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub cache_ttl_seconds: u64,
    pub last_error: Option<String>,
}

#[cfg(test)]
#[path = "skill_dtos_tests.rs"]
mod tests;

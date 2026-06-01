//! MCP and model profile settings DTOs.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::EffectiveItem;

// -- agent settings DTOs --

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum AgentSettingsScope {
    Builtin,
    User,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AgentSettingsInput {
    pub scope: AgentSettingsScope,
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    #[serde(rename = "modelProfile")]
    pub model_profile: Option<String>,
    #[serde(default, rename = "reasoningEffort")]
    pub reasoning_effort: Option<String>,
    pub skills: Vec<String>,
    #[serde(rename = "nicknameCandidates")]
    pub nickname_candidates: Vec<String>,
    pub enabled: bool,
    pub instructions: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AgentSettingsView {
    #[serde(rename = "settingsId")]
    pub settings_id: String,
    pub name: String,
    pub description: String,
    pub scope: AgentSettingsScope,
    pub path: String,
    pub tools: Vec<String>,
    #[serde(rename = "modelProfile")]
    pub model_profile: Option<String>,
    #[serde(default, rename = "reasoningEffort")]
    pub reasoning_effort: Option<String>,
    pub skills: Vec<String>,
    #[serde(rename = "nicknameCandidates")]
    pub nickname_candidates: Vec<String>,
    pub enabled: bool,
    pub instructions: String,
    pub effective: bool,
    #[serde(rename = "shadowedBy")]
    pub shadowed_by: Option<String>,
    pub valid: bool,
    #[serde(rename = "validationError")]
    pub validation_error: Option<String>,
    pub editable: bool,
    pub deletable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EffectiveAgentView {
    pub value: AgentSettingsView,
    pub source: crate::config_scope::ConfigScope,
    pub overrides: Option<crate::config_scope::ConfigScope>,
    pub enabled: bool,
    #[serde(rename = "disabledBy")]
    pub disabled_by: Option<crate::config_scope::ConfigScope>,
    pub writable: bool,
    pub deletable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpServerSettingsTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: BTreeMap<String, String>,
    },
    Sse {
        url: String,
        headers: BTreeMap<String, String>,
    },
    StreamableHttp {
        url: String,
        headers: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpServerSettingsInput {
    pub name: String,
    pub transport: McpServerSettingsTransport,
    pub enabled: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpServerSettingsView {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub runtime_status: String,
    pub trusted: bool,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub tool_count: Option<usize>,
    pub last_error: Option<String>,
    pub writable: bool,
    pub config_path: Option<String>,
    pub description: Option<String>,
    pub source: String,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub diagnostic_summary: String,
}

impl McpServerSettingsView {
    pub fn refresh_diagnostic_summary(&mut self) {
        let trust = if self.trusted { "trusted" } else { "untrusted" };
        let tools = match self.tool_count {
            Some(1) => "1 tool".to_string(),
            Some(count) => format!("{count} tools"),
            None => "unknown".to_string(),
        };
        let verification = if self.verified {
            "verified"
        } else {
            "unverified"
        };
        let error = self.last_error.as_deref().unwrap_or("none");
        self.diagnostic_summary = format!(
            "status: {}; trust: {trust}; tools: {tools}; {verification}; error: {error}",
            self.runtime_status
        );
    }
}

/// Concrete effective-view wrapper for MCP server settings.
/// Combines [`EffectiveItem`] metadata with a [`McpServerSettingsView`].
/// This is a non-generic type so it can safely derive both serde and specta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EffectiveMcpServerView {
    pub value: McpServerSettingsView,
    pub source: crate::config_scope::ConfigScope,
    pub overrides: Option<crate::config_scope::ConfigScope>,
    pub enabled: bool,
    #[serde(rename = "disabledBy")]
    pub disabled_by: Option<crate::config_scope::ConfigScope>,
    pub writable: bool,
    pub deletable: bool,
}

impl EffectiveMcpServerView {
    pub fn from_effective(item: EffectiveItem<McpServerSettingsView>) -> Self {
        Self {
            value: item.value,
            source: item.source,
            overrides: item.overrides,
            enabled: item.enabled,
            disabled_by: item.disabled_by,
            writable: item.writable,
            deletable: item.deletable,
        }
    }
}

/// Concrete effective-view wrapper for profile settings.
/// Combines [`EffectiveItem`] metadata with a [`ProfileSettingsView`].
/// This is a non-generic type so it can safely derive both serde and specta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EffectiveProfileView {
    pub value: ProfileSettingsView,
    pub source: crate::config_scope::ConfigScope,
    pub overrides: Option<crate::config_scope::ConfigScope>,
    pub enabled: bool,
    #[serde(rename = "disabledBy")]
    pub disabled_by: Option<crate::config_scope::ConfigScope>,
    pub writable: bool,
    pub deletable: bool,
}

impl EffectiveProfileView {
    pub fn from_effective(item: EffectiveItem<ProfileSettingsView>) -> Self {
        Self {
            value: item.value,
            source: item.source,
            overrides: item.overrides,
            enabled: item.enabled,
            disabled_by: item.disabled_by,
            writable: item.writable,
            deletable: item.deletable,
        }
    }

    pub fn from_view(view: ProfileSettingsView, source: crate::config_scope::ConfigScope) -> Self {
        let enabled = view.enabled;
        let mut item = EffectiveItem::new(view, source);
        item.enabled = enabled;
        Self::from_effective(item)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProfileSettingsInput {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub context_window: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub output_limit: Option<u64>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub max_tokens: Option<u64>,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub client_identity: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProfileSettingsView {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub enabled: bool,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub context_window: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub output_limit: Option<u64>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub max_tokens: Option<u64>,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub client_identity: Option<String>,
    pub has_api_key: bool,
    pub writable: bool,
    pub config_path: Option<String>,
    pub source: String,
}

// -- instructions settings DTOs --

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstructionsView {
    pub system: String,
    pub user: Option<String>,
    pub project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstructionsUpdateInput {
    pub scope: crate::config_scope::ConfigScope,
    pub text: String,
}

// -- hooks settings DTOs --

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct HookSettingsView {
    pub id: String,
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
    #[serde(rename = "statusMessage")]
    pub status_message: Option<String>,
    #[serde(rename = "timeoutSecs")]
    pub timeout_secs: Option<u32>,
    pub enabled: bool,
    pub source: crate::config_scope::ConfigScope,
    #[serde(rename = "configPath")]
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct HookSettingsInput {
    pub scope: crate::config_scope::ConfigScope,
    pub id: String,
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
    #[serde(rename = "statusMessage")]
    pub status_message: Option<String>,
    #[serde(rename = "timeoutSecs")]
    pub timeout_secs: Option<u32>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct HookTemplateView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
    #[serde(rename = "statusMessage")]
    pub status_message: Option<String>,
    #[serde(rename = "timeoutSecs")]
    pub timeout_secs: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct HooksSettingsView {
    pub user: Vec<HookSettingsView>,
    pub project: Vec<HookSettingsView>,
    pub templates: Vec<HookTemplateView>,
    #[serde(rename = "userConfigPath")]
    pub user_config_path: String,
    #[serde(rename = "projectConfigPath")]
    pub project_config_path: Option<String>,
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;

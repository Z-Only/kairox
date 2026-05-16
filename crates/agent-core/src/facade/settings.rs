//! MCP and model profile settings DTOs.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::EffectiveItem;

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
    pub fn from_view(view: ProfileSettingsView, source: crate::config_scope::ConfigScope) -> Self {
        Self {
            writable: source >= crate::config_scope::ConfigScope::User,
            deletable: source >= crate::config_scope::ConfigScope::User,
            enabled: view.enabled,
            value: view,
            source,
            overrides: None,
            disabled_by: None,
        }
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
    pub has_api_key: bool,
    pub writable: bool,
    pub config_path: Option<String>,
    pub source: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_settings_input_serializes_stdio_transport() {
        let input = McpServerSettingsInput {
            name: "filesystem".to_string(),
            transport: McpServerSettingsTransport::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-filesystem".to_string(),
                ],
                env: BTreeMap::from([("ROOT".to_string(), "/tmp".to_string())]),
            },
            enabled: true,
            description: Some("Local files".to_string()),
        };

        let encoded = serde_json::to_string(&input).expect("input should serialize");
        assert!(encoded.contains("filesystem"));
        assert!(encoded.contains("stdio"));
    }
}

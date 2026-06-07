//! TOML parsing, API key resolution, and validation.

mod catalog;
mod env;
mod lsp;
mod mcp;
mod overlay;
mod profile;

use crate::{Config, ConfigError};

pub use catalog::{
    default_catalog_sources, load_with_marketplace_loaded, merge_with_defaults,
    parse_catalog_sources, CatalogSourceConfig, CatalogSourceKind, LoadedConfig,
};
pub use env::{resolve_api_keys, resolve_mcp_env};
pub use overlay::load_with_marketplace_overlay;
pub use profile::validate;

/// Intermediate TOML structure for deserialization.
#[derive(Debug, serde::Deserialize)]
struct ConfigToml {
    #[serde(default)]
    features: crate::FeatureFlags,
    #[serde(default)]
    profiles: toml::value::Table,
    #[serde(default)]
    mcp_servers: toml::value::Table,
    #[serde(default)]
    context: crate::ContextPolicy,
    /// Top-level list of MCP server IDs to disable at this config layer.
    #[serde(default)]
    disabled_mcp_servers: Vec<String>,
    /// Optional custom instructions appended to the system prompt at this layer.
    #[serde(default)]
    instructions: Option<String>,
    #[serde(default)]
    hooks: toml::value::Table,
    #[serde(default)]
    lsp_servers: toml::value::Table,
    #[serde(default)]
    dap_servers: toml::value::Table,
    #[serde(default)]
    advisor: Option<crate::AdvisorConfig>,
}

/// Parse a TOML string into a Config.
pub fn load_from_str(content: &str, path_for_errors: &str) -> Result<Config, ConfigError> {
    let config_toml: ConfigToml = toml::from_str(content).map_err(|e| ConfigError::Parse {
        path: path_for_errors.to_string(),
        message: e.to_string(),
    })?;

    Ok(Config {
        profiles: profile::parse_profiles(&config_toml.profiles, path_for_errors)?,
        mcp_servers: mcp::parse_mcp_servers(&config_toml.mcp_servers, path_for_errors)?,
        source: crate::ConfigSource::ProjectFile, // Will be overridden by caller
        context: config_toml.context,
        disabled_mcp_servers: config_toml.disabled_mcp_servers,
        instructions: config_toml.instructions,
        features: config_toml.features,
        hooks: parse_hooks(&config_toml.hooks, path_for_errors)?,
        lsp_servers: lsp::parse_lsp_servers(&config_toml.lsp_servers, path_for_errors)?,
        dap_servers: lsp::parse_dap_servers(&config_toml.dap_servers, path_for_errors)?,
        advisor: config_toml.advisor.unwrap_or_default(),
    })
}

fn parse_hooks(
    table: &toml::value::Table,
    path_for_errors: &str,
) -> Result<Vec<crate::HookConfig>, ConfigError> {
    let mut hooks = Vec::new();
    for (event_name, event_value) in table {
        let event = crate::HookEvent::parse(event_name).ok_or_else(|| ConfigError::Parse {
            path: path_for_errors.to_string(),
            message: format!("unsupported hook event '{event_name}'"),
        })?;
        let event_table = event_value.as_table().ok_or_else(|| ConfigError::Parse {
            path: path_for_errors.to_string(),
            message: format!("hooks.{event_name} must be a table"),
        })?;
        for (id, hook_value) in event_table {
            let hook: crate::HookConfigToml =
                hook_value
                    .clone()
                    .try_into()
                    .map_err(|e: toml::de::Error| ConfigError::Parse {
                        path: path_for_errors.to_string(),
                        message: format!("invalid hook '{event_name}.{id}': {e}"),
                    })?;
            hooks.push(hook.into_hook_config(event, id.clone()));
        }
    }
    Ok(hooks)
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;

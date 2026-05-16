//! TOML parsing, API key resolution, and validation.

mod catalog;
mod env;
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
    profiles: toml::value::Table,
    #[serde(default)]
    mcp_servers: toml::value::Table,
    #[serde(default)]
    context: crate::ContextPolicy,
    /// Top-level list of MCP server IDs to disable at this config layer.
    #[serde(default)]
    disabled_mcp_servers: Vec<String>,
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_context_policy_with_defaults_and_overrides() {
        // Defaults: omitting [context] yields the default ContextPolicy.
        let cfg_default: crate::Config = crate::loader::load_from_str(
            r#"
[profiles.fake]
provider = "fake"
model_id = "fake"
"#,
            "test.toml",
        )
        .unwrap();
        assert!(
            (cfg_default.context.auto_compact_threshold - 0.85_f32).abs() < 1e-6,
            "default threshold should be 0.85, got {}",
            cfg_default.context.auto_compact_threshold
        );
        assert!(cfg_default.context.compactor_profile.is_none());
        assert!(cfg_default.context.max_tool_definition_tokens.is_none());

        // Overrides: explicit values take precedence.
        let cfg_user: crate::Config = crate::loader::load_from_str(
            r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4o"
base_url = "https://api.openai.com/v1"

[context]
auto_compact_threshold = 0.7
compactor_profile = "fast"
max_tool_definition_tokens = 25000
"#,
            "test.toml",
        )
        .unwrap();
        assert!((cfg_user.context.auto_compact_threshold - 0.7).abs() < 1e-6);
        assert_eq!(cfg_user.context.compactor_profile.as_deref(), Some("fast"));
        assert_eq!(cfg_user.context.max_tool_definition_tokens, Some(25_000));
    }

    #[test]
    fn parse_error_on_invalid_toml() {
        let toml = "this is not valid toml {{{{";
        let result = load_from_str(toml, "bad.toml");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::Parse { path, .. } => assert_eq!(path, "bad.toml"),
            _ => panic!("expected Parse error"),
        }
    }

    #[test]
    fn config_parse_includes_context_policy() {
        // Empty [context] section uses defaults.
        let cfg_empty: crate::Config = crate::loader::load_from_str(
            r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[context]
"#,
            "test.toml",
        )
        .unwrap();
        assert!(
            (cfg_empty.context.auto_compact_threshold - 0.85_f32).abs() < 1e-6,
            "default should be 0.85"
        );
        assert!(cfg_empty.context.compactor_profile.is_none());
        assert!(cfg_empty.context.max_tool_definition_tokens.is_none());

        // Override works.
        let cfg_override: crate::Config = crate::loader::load_from_str(
            r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[context]
auto_compact_threshold = 0.9
compactor_profile = "fake"
max_tool_definition_tokens = 50000
"#,
            "test.toml",
        )
        .unwrap();
        assert!((cfg_override.context.auto_compact_threshold - 0.9).abs() < 1e-6);
        assert_eq!(
            cfg_override.context.compactor_profile.as_deref(),
            Some("fake")
        );
        assert_eq!(
            cfg_override.context.max_tool_definition_tokens,
            Some(50_000)
        );
    }
}

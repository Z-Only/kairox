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

    #[test]
    fn config_parse_includes_hooks() {
        let cfg: crate::Config = crate::loader::load_from_str(
            r#"
[features]
hooks = false

[hooks.Stop.verify]
matcher = "*"
command = "cargo test --workspace --all-targets"
status_message = "Running workspace tests"
timeout_secs = 120
enabled = true

[hooks.PreToolUse.block_rm]
matcher = "shell"
command = "python3 .kairox/hooks/pre_tool.py"
enabled = false
"#,
            "test.toml",
        )
        .unwrap();

        assert!(!cfg.features.hooks);
        assert_eq!(cfg.hooks.len(), 2);
        let verify = cfg
            .hooks
            .iter()
            .find(|hook| hook.event == crate::HookEvent::Stop && hook.id == "verify")
            .expect("Stop.verify hook should parse");
        assert_eq!(verify.matcher.as_deref(), Some("*"));
        assert_eq!(verify.command, "cargo test --workspace --all-targets");
        assert_eq!(
            verify.status_message.as_deref(),
            Some("Running workspace tests")
        );
        assert_eq!(verify.timeout_secs, Some(120));
        assert!(verify.enabled);
        let pre_tool = cfg
            .hooks
            .iter()
            .find(|hook| hook.event == crate::HookEvent::PreToolUse && hook.id == "block_rm")
            .expect("PreToolUse.block_rm hook should parse");
        assert!(!pre_tool.enabled);
    }
}

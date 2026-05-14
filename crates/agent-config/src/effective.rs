use std::collections::HashSet;

use crate::{Config, ConfigSource, ProfileDef};
use agent_core::config_scope::ConfigScope;
use agent_core::EffectiveItem;

/// Convert MCP server configs into `EffectiveItem` wrappers annotated with their source scope.
pub fn build_effective_mcp_servers(config: &Config) -> Vec<EffectiveItem<agent_mcp::McpServerDef>> {
    let source = config_source_to_scope(&config.source);
    let disabled: HashSet<&str> = config
        .disabled_mcp_servers
        .iter()
        .map(|s| s.as_str())
        .collect();

    config
        .mcp_servers
        .iter()
        .map(|(name, def)| {
            let server_def = def.to_server_def(name);
            let mut item = EffectiveItem::new(server_def, source);
            if disabled.contains(name.as_str()) {
                item = item.with_disabled(ConfigScope::Project);
            }
            item
        })
        .collect()
}

/// Convert profile definitions into `EffectiveItem` wrappers annotated with their source scope.
pub fn build_effective_profiles(config: &Config) -> Vec<EffectiveItem<ProfileDef>> {
    let source = config_source_to_scope(&config.source);

    config
        .profiles
        .iter()
        .map(|(_alias, profile)| {
            let p = profile.clone();
            EffectiveItem::new(p, source)
        })
        .collect()
}

/// Map `ConfigSource` to the corresponding `ConfigScope`.
fn config_source_to_scope(source: &ConfigSource) -> ConfigScope {
    match source {
        ConfigSource::ProjectFile => ConfigScope::Project,
        ConfigSource::UserFile => ConfigScope::User,
        ConfigSource::LocalFile => ConfigScope::Local,
        ConfigSource::Defaults => ConfigScope::Builtin,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_mcp_empty_config() {
        let config = Config::defaults();
        let servers = build_effective_mcp_servers(&config);
        assert!(servers.is_empty());
    }

    #[test]
    fn disabled_mcp_server_marked_in_effective_view() {
        let mut config = Config::defaults();
        config.mcp_servers.push((
            "files".to_string(),
            crate::McpServerConfig {
                r#type: crate::McpTransportType::Stdio,
                command: Some("echo".to_string()),
                args: Some(vec![]),
                env: None,
                cwd: None,
                url: None,
                headers: None,
                api_key_env: None,
                keep_alive: false,
                idle_timeout_secs: 300,
                auto_restart: true,
                max_restart_attempts: 3,
            },
        ));
        config.disabled_mcp_servers = vec!["files".to_string()];

        let servers = build_effective_mcp_servers(&config);
        assert_eq!(servers.len(), 1);
        assert!(!servers[0].enabled);
        assert_eq!(servers[0].disabled_by, Some(ConfigScope::Project));
    }

    #[test]
    fn non_disabled_server_not_affected() {
        let mut config = Config::defaults();
        config.mcp_servers.push((
            "files".to_string(),
            crate::McpServerConfig {
                r#type: crate::McpTransportType::Stdio,
                command: Some("echo".to_string()),
                args: Some(vec![]),
                env: None,
                cwd: None,
                url: None,
                headers: None,
                api_key_env: None,
                keep_alive: false,
                idle_timeout_secs: 300,
                auto_restart: true,
                max_restart_attempts: 3,
            },
        ));
        config.disabled_mcp_servers = vec![];

        let servers = build_effective_mcp_servers(&config);
        assert_eq!(servers.len(), 1);
        assert!(servers[0].enabled);
        assert_eq!(servers[0].disabled_by, None);
    }
}

use crate::{Config, ConfigSource, ProfileDef};
use agent_core::config_scope::ConfigScope;
use agent_core::EffectiveItem;

/// Convert MCP server configs into `EffectiveItem` wrappers annotated with their source scope.
pub fn build_effective_mcp_servers(config: &Config) -> Vec<EffectiveItem<agent_mcp::McpServerDef>> {
    let source = config_source_to_scope(&config.source);

    config
        .mcp_servers
        .iter()
        .map(|(name, def)| {
            let server_def = def.to_server_def(name);
            EffectiveItem::new(server_def, source)
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
}

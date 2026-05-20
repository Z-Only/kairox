use std::collections::HashSet;

use crate::{Config, ConfigSource, ProfileDef};
use agent_core::config_scope::ConfigScope;
use agent_core::facade::{McpServerSettingsView, ProfileSettingsView};
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

/// Convert MCP settings views into effective wrappers with shared source and disabled metadata.
pub fn build_effective_mcp_server_settings_views(
    views: Vec<McpServerSettingsView>,
    disabled_mcp_servers: &[String],
) -> Vec<EffectiveItem<McpServerSettingsView>> {
    let disabled: HashSet<&str> = disabled_mcp_servers.iter().map(String::as_str).collect();

    views
        .into_iter()
        .map(|view| {
            let source = settings_source_to_scope(&view.source);
            let enabled = view.enabled;
            let writable = view.writable;
            let id = view.id.clone();
            let mut item = EffectiveItem::new(view, source);
            item.enabled = enabled;
            item.writable = writable;
            item.deletable = writable;
            if disabled.contains(id.as_str()) {
                item = item.with_disabled(ConfigScope::Project);
            }
            item
        })
        .collect()
}

/// Convert profile definitions into settings DTOs wrapped with effective metadata.
pub fn build_effective_profile_settings_views(
    config: &Config,
) -> Vec<EffectiveItem<ProfileSettingsView>> {
    let source = config_source_to_scope(&config.source);

    config
        .profiles
        .iter()
        .map(|(alias, profile)| {
            let view = ProfileSettingsView {
                alias: alias.clone(),
                provider: profile.provider.clone(),
                model_id: profile.model_id.clone(),
                enabled: profile.enabled,
                context_window: profile.context_window,
                output_limit: profile.output_limit,
                temperature: profile.temperature,
                top_p: profile.top_p,
                top_k: profile.top_k,
                max_tokens: profile.max_tokens,
                base_url: profile.base_url.clone(),
                api_key_env: profile.api_key_env.clone(),
                has_api_key: profile_has_api_key(profile),
                writable: source >= ConfigScope::User,
                config_path: None,
                source: source.to_string(),
            };
            let mut item = EffectiveItem::new(view, source);
            item.enabled = profile.enabled;
            item
        })
        .collect()
}

/// Map `ConfigSource` to the corresponding `ConfigScope`.
pub fn config_source_to_scope(source: &ConfigSource) -> ConfigScope {
    match source {
        ConfigSource::ProjectFile => ConfigScope::Project,
        ConfigSource::UserFile => ConfigScope::User,
        ConfigSource::LocalFile => ConfigScope::Local,
        ConfigSource::Defaults => ConfigScope::Builtin,
    }
}

/// Map settings row source labels to the corresponding `ConfigScope`.
pub fn settings_source_to_scope(source: &str) -> ConfigScope {
    match source {
        "project" | "project_config" => ConfigScope::Project,
        "local" | "local_config" => ConfigScope::Local,
        "user" | "user_config" | "profiles_toml" => ConfigScope::User,
        "builtin" | "defaults" => ConfigScope::Builtin,
        _ => ConfigScope::Builtin,
    }
}

fn profile_has_api_key(profile: &ProfileDef) -> bool {
    profile.api_key.is_some()
        || profile
            .api_key_env
            .as_deref()
            .is_some_and(|env| std::env::var(env).is_ok())
        || matches!(profile.provider.as_str(), "ollama" | "fake")
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

    #[test]
    fn effective_settings_views_map_source_disabled_and_direct_api_keys() {
        let mcp_views = vec![McpServerSettingsView {
            id: "files".to_string(),
            name: "files".to_string(),
            transport: "stdio".to_string(),
            enabled: true,
            runtime_status: "stopped".to_string(),
            trusted: false,
            tool_count: None,
            last_error: None,
            writable: true,
            config_path: Some("/tmp/config.toml".to_string()),
            description: None,
            source: "user_config".to_string(),
            verified: true,
        }];

        let effective_mcp =
            build_effective_mcp_server_settings_views(mcp_views, &["files".to_string()]);
        assert_eq!(effective_mcp.len(), 1);
        assert_eq!(effective_mcp[0].source, ConfigScope::User);
        assert!(!effective_mcp[0].enabled);
        assert_eq!(effective_mcp[0].disabled_by, Some(ConfigScope::Project));

        let mut config = Config::defaults();
        config.source = ConfigSource::LocalFile;
        config.profiles = vec![(
            "direct".to_string(),
            ProfileDef {
                provider: "openai_compatible".to_string(),
                model_id: "gpt-4.1".to_string(),
                base_url: Some("https://api.openai.com/v1".to_string()),
                api_key: Some("sk-direct".to_string()),
                api_key_env: Some("KAIROX_DIRECT_KEY_SHOULD_NOT_BE_READ".to_string()),
                context_window: Some(128_000),
                output_limit: Some(16_384),
                response: None,
                max_tokens: None,
                temperature: None,
                top_p: None,
                top_k: None,
                headers: None,
                supports_tools: None,
                supports_vision: None,
                supports_reasoning: None,
                extra_params: None,
                enabled: true,
            },
        )];

        let effective_profiles = build_effective_profile_settings_views(&config);
        assert_eq!(effective_profiles.len(), 1);
        assert_eq!(effective_profiles[0].source, ConfigScope::Local);
        assert_eq!(effective_profiles[0].value.source, "local");
        assert!(effective_profiles[0].value.has_api_key);
    }
}

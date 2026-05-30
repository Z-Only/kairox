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
        diagnostic_summary: String::new(),
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

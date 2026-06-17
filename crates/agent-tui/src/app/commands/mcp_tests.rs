use super::*;

fn temp_config_path(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "kairox-tui-mcp-tests-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    (dir.join(".kairox/config.toml"), dir)
}

fn setting(id: &str, enabled: bool) -> agent_core::facade::McpServerSettingsView {
    agent_core::facade::McpServerSettingsView {
        id: id.to_string(),
        name: id.to_string(),
        transport: "stdio".to_string(),
        enabled,
        runtime_status: "stopped".to_string(),
        trusted: false,
        tool_count: None,
        last_error: None,
        writable: true,
        config_path: None,
        description: None,
        source: "project".to_string(),
        verified: false,
        diagnostic_summary: String::new(),
    }
}

#[test]
fn apply_mcp_scope_disabled_creates_project_config_with_sorted_ids() {
    let (config_path, dir) = temp_config_path("creates-sorted");

    apply_mcp_scope_disabled(&config_path, "zeta", true).expect("disable zeta");
    apply_mcp_scope_disabled(&config_path, "alpha", true).expect("disable alpha");
    apply_mcp_scope_disabled(&config_path, "zeta", true).expect("disable zeta again");

    let raw = std::fs::read_to_string(&config_path).expect("config should be written");
    assert!(raw.contains(r#"disabled_mcp_servers = ["alpha", "zeta"]"#));
    let disabled = read_disabled_mcp_scope(&config_path).expect("disabled ids should parse");
    assert_eq!(
        disabled,
        ["alpha".to_string(), "zeta".to_string()]
            .into_iter()
            .collect()
    );

    std::fs::remove_dir_all(dir).expect("cleanup temp dir");
}

#[test]
fn apply_mcp_scope_disabled_preserves_existing_toml_and_removes_key_when_empty() {
    let (config_path, dir) = temp_config_path("preserves-removes");
    std::fs::create_dir_all(config_path.parent().expect("config parent")).expect("mkdir");
    std::fs::write(
        &config_path,
        r#"
profile = "local"
disabled_mcp_servers = ["alpha"]

[models.default]
name = "gpt"
"#,
    )
    .expect("seed config");

    apply_mcp_scope_disabled(&config_path, "beta", true).expect("disable beta");
    apply_mcp_scope_disabled(&config_path, "alpha", false).expect("enable alpha");
    let raw = std::fs::read_to_string(&config_path).expect("config should be readable");
    assert!(raw.contains(r#"profile = "local""#));
    assert!(raw.contains("[models.default]"));
    assert!(raw.contains(r#"disabled_mcp_servers = ["beta"]"#));

    apply_mcp_scope_disabled(&config_path, "beta", false).expect("enable beta");
    let raw = std::fs::read_to_string(&config_path).expect("config should be readable");
    assert!(raw.contains(r#"profile = "local""#));
    assert!(raw.contains("[models.default]"));
    assert!(!raw.contains("disabled_mcp_servers"));

    std::fs::remove_dir_all(dir).expect("cleanup temp dir");
}

#[test]
fn read_disabled_mcp_scope_returns_empty_for_missing_config_and_error_for_invalid_toml() {
    let (config_path, dir) = temp_config_path("read-errors");
    let disabled = read_disabled_mcp_scope(&config_path).expect("missing config should be empty");
    assert!(disabled.is_empty());

    std::fs::create_dir_all(config_path.parent().expect("config parent")).expect("mkdir");
    std::fs::write(&config_path, "disabled_mcp_servers = [").expect("seed invalid config");
    let error = read_disabled_mcp_scope(&config_path).expect_err("invalid TOML should error");
    assert!(error.contains("failed to parse project config"));

    std::fs::remove_dir_all(dir).expect("cleanup temp dir");
}

#[test]
fn apply_project_disabled_scope_only_disables_matching_servers() {
    let mut settings = vec![
        setting("alpha", true),
        setting("beta", true),
        setting("gamma", false),
    ];
    let disabled = ["beta".to_string(), "missing".to_string()]
        .into_iter()
        .collect();

    apply_project_disabled_scope(&mut settings, &disabled);

    assert!(settings[0].enabled);
    assert!(!settings[1].enabled);
    assert!(!settings[2].enabled);
}

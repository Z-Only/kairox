use super::*;
use serde_json::json;
use std::collections::HashMap;

#[test]
fn lsp_config_deserializes_defaults() {
    let cfg: LspServerConfig = toml::from_str(r#"command = "rust-analyzer""#).unwrap();

    assert_eq!(cfg.command, "rust-analyzer");
    assert!(cfg.args.is_empty());
    assert!(cfg.env.is_empty());
    assert!(cfg.cwd.is_none());
    assert!(cfg.languages.is_empty());
    assert!(cfg.file_patterns.is_empty());
    assert!(cfg.initialization_options.is_none());
    assert!(cfg.auto_start);
}

#[test]
fn lsp_config_to_server_def_preserves_runtime_fields() {
    let mut env = HashMap::new();
    env.insert("RA_LOG".to_string(), "info".to_string());
    let cfg = LspServerConfig {
        command: "rust-analyzer".to_string(),
        args: vec!["--stdio".to_string()],
        env,
        cwd: Some("/workspace".to_string()),
        languages: vec!["rust".to_string()],
        file_patterns: vec!["*.rs".to_string()],
        initialization_options: Some(json!({ "check": { "command": "clippy" } })),
        auto_start: false,
    };

    let def = cfg.to_server_def("ra");

    assert_eq!(def.name, "ra");
    assert_eq!(def.command, "rust-analyzer");
    assert_eq!(def.args, vec!["--stdio"]);
    assert_eq!(def.env.get("RA_LOG").map(String::as_str), Some("info"));
    assert_eq!(def.cwd.as_deref(), Some("/workspace"));
    assert_eq!(def.languages, vec!["rust"]);
    assert_eq!(def.file_patterns, vec!["*.rs"]);
    assert_eq!(
        def.initialization_options
            .as_ref()
            .and_then(|value| value.pointer("/check/command"))
            .and_then(serde_json::Value::as_str),
        Some("clippy")
    );
}

#[test]
fn dap_config_deserializes_defaults() {
    let cfg: DapServerConfig = toml::from_str(r#"command = "codelldb""#).unwrap();

    assert_eq!(cfg.command, "codelldb");
    assert!(cfg.args.is_empty());
    assert!(cfg.env.is_empty());
    assert!(cfg.cwd.is_none());
    assert!(cfg.languages.is_empty());
}

#[test]
fn dap_config_to_server_def_preserves_runtime_fields() {
    let mut env = HashMap::new();
    env.insert("RUST_LOG".to_string(), "debug".to_string());
    let cfg = DapServerConfig {
        command: "codelldb".to_string(),
        args: vec!["--port".to_string(), "0".to_string()],
        env,
        cwd: Some("/workspace".to_string()),
        languages: vec!["rust".to_string()],
    };

    let def = cfg.to_server_def("lldb");

    assert_eq!(def.name, "lldb");
    assert_eq!(def.command, "codelldb");
    assert_eq!(def.args, vec!["--port", "0"]);
    assert_eq!(def.env.get("RUST_LOG").map(String::as_str), Some("debug"));
    assert_eq!(def.cwd.as_deref(), Some("/workspace"));
    assert_eq!(def.languages, vec!["rust"]);
}

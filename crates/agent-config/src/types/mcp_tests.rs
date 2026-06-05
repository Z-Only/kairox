use super::*;
use std::collections::HashMap;

// ── McpTransportType serde ──────────────────────────────────────────

#[test]
fn transport_type_serde_roundtrip_json() {
    let variants = [
        (McpTransportType::Stdio, r#""stdio""#),
        (McpTransportType::Sse, r#""sse""#),
        (McpTransportType::StreamableHttp, r#""streamable_http""#),
    ];
    for (variant, expected_json) in variants {
        let json = serde_json::to_string(&variant).expect("serialize");
        assert_eq!(json, expected_json);
        let back: McpTransportType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, variant);
    }
}

#[test]
fn transport_type_equality() {
    assert_eq!(McpTransportType::Stdio, McpTransportType::Stdio);
    assert_ne!(McpTransportType::Stdio, McpTransportType::Sse);
}

// ── McpServerConfig defaults ────────────────────────────────────────

#[test]
fn default_idle_timeout_is_300() {
    assert_eq!(default_idle_timeout(), 300);
}

#[test]
fn default_max_restart_attempts_is_3() {
    assert_eq!(default_max_restart_attempts(), 3);
}

// ── McpServerConfig serde ───────────────────────────────────────────

#[test]
fn minimal_stdio_config() {
    let toml_str = r#"
        type = "stdio"
        command = "node"
        args = ["server.js"]
    "#;
    let cfg: McpServerConfig = toml::from_str(toml_str).expect("parse");
    assert_eq!(cfg.r#type, McpTransportType::Stdio);
    assert_eq!(cfg.command.as_deref(), Some("node"));
    assert_eq!(cfg.args.as_deref(), Some(&["server.js".to_string()][..]));
    assert!(cfg.url.is_none());
    assert!(cfg.headers.is_none());
    assert!(!cfg.keep_alive);
    assert_eq!(cfg.idle_timeout_secs, 300);
    assert!(cfg.auto_restart);
    assert_eq!(cfg.max_restart_attempts, 3);
}

#[test]
fn sse_config_with_all_fields() {
    let toml_str = r#"
        type = "sse"
        url = "https://mcp.example.com/sse"
        api_key_env = "MCP_KEY"
        keep_alive = true
        idle_timeout_secs = 600
        auto_restart = false
        max_restart_attempts = 5

        [headers]
        Authorization = "Bearer token"
    "#;
    let cfg: McpServerConfig = toml::from_str(toml_str).expect("parse");
    assert_eq!(cfg.r#type, McpTransportType::Sse);
    assert_eq!(cfg.url.as_deref(), Some("https://mcp.example.com/sse"));
    assert_eq!(cfg.api_key_env.as_deref(), Some("MCP_KEY"));
    assert!(cfg.keep_alive);
    assert_eq!(cfg.idle_timeout_secs, 600);
    assert!(!cfg.auto_restart);
    assert_eq!(cfg.max_restart_attempts, 5);
    let hdrs = cfg.headers.as_ref().expect("headers");
    assert_eq!(hdrs.get("Authorization").map(|s| s.as_str()), Some("Bearer token"));
}

#[test]
fn streamable_http_config() {
    let toml_str = r#"
        type = "streamable_http"
        url = "https://mcp.example.com/stream"
    "#;
    let cfg: McpServerConfig = toml::from_str(toml_str).expect("parse");
    assert_eq!(cfg.r#type, McpTransportType::StreamableHttp);
    assert_eq!(cfg.url.as_deref(), Some("https://mcp.example.com/stream"));
}

// ── to_server_def conversion ────────────────────────────────────────

#[test]
fn stdio_to_server_def() {
    let mut env = HashMap::new();
    env.insert("FOO".into(), "bar".into());
    let cfg = McpServerConfig {
        r#type: McpTransportType::Stdio,
        command: Some("python".into()),
        args: Some(vec!["-m".into(), "server".into()]),
        env: Some(env),
        cwd: Some("/tmp".into()),
        url: None,
        headers: None,
        api_key_env: None,
        keep_alive: true,
        idle_timeout_secs: 120,
        auto_restart: false,
        max_restart_attempts: 1,
    };
    let def = cfg.to_server_def("my-server");
    assert_eq!(def.name, "my-server");
    assert_eq!(def.args, vec!["-m", "server"]);
    assert_eq!(def.env.get("FOO").map(|s| s.as_str()), Some("bar"));
    assert!(def.keep_alive);
    assert_eq!(def.idle_timeout_secs, 120);
    assert!(!def.auto_restart);
    assert_eq!(def.max_restart_attempts, 1);

    match &def.transport {
        agent_mcp::McpTransportDef::Stdio { command, cwd } => {
            assert_eq!(command, "python");
            assert_eq!(cwd.as_deref(), Some("/tmp"));
        }
        other => panic!("expected Stdio, got {other:?}"),
    }
}

#[test]
fn sse_to_server_def() {
    let mut headers = HashMap::new();
    headers.insert("X-Key".into(), "val".into());
    let cfg = McpServerConfig {
        r#type: McpTransportType::Sse,
        command: None,
        args: None,
        env: None,
        cwd: None,
        url: Some("https://example.com/sse".into()),
        headers: Some(headers),
        api_key_env: Some("SSE_KEY".into()),
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: true,
        max_restart_attempts: 3,
    };
    let def = cfg.to_server_def("sse-srv");
    assert_eq!(def.name, "sse-srv");
    match &def.transport {
        agent_mcp::McpTransportDef::Sse {
            url,
            api_key_env,
            headers,
        } => {
            assert_eq!(url, "https://example.com/sse");
            assert_eq!(api_key_env.as_deref(), Some("SSE_KEY"));
            assert_eq!(headers.get("X-Key").map(|s| s.as_str()), Some("val"));
        }
        other => panic!("expected Sse, got {other:?}"),
    }
}

#[test]
fn streamable_http_to_server_def() {
    let cfg = McpServerConfig {
        r#type: McpTransportType::StreamableHttp,
        command: None,
        args: None,
        env: None,
        cwd: None,
        url: Some("https://example.com/stream".into()),
        headers: None,
        api_key_env: None,
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: true,
        max_restart_attempts: 3,
    };
    let def = cfg.to_server_def("stream-srv");
    match &def.transport {
        agent_mcp::McpTransportDef::StreamableHttp {
            url,
            api_key_env,
            headers,
        } => {
            assert_eq!(url, "https://example.com/stream");
            assert!(api_key_env.is_none());
            assert!(headers.is_empty());
        }
        other => panic!("expected StreamableHttp, got {other:?}"),
    }
}

#[test]
fn missing_command_defaults_to_empty_string_in_stdio() {
    let cfg = McpServerConfig {
        r#type: McpTransportType::Stdio,
        command: None,
        args: None,
        env: None,
        cwd: None,
        url: None,
        headers: None,
        api_key_env: None,
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: true,
        max_restart_attempts: 3,
    };
    let def = cfg.to_server_def("empty");
    match &def.transport {
        agent_mcp::McpTransportDef::Stdio { command, .. } => {
            assert_eq!(command, "");
        }
        other => panic!("expected Stdio, got {other:?}"),
    }
}

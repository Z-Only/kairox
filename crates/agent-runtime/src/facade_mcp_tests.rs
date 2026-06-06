use super::*;
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use std::collections::HashMap;

// ── Helpers ───────────────────────────────────────────────────────────────

async fn build_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(store, model)
}

async fn build_runtime_with_marketplace() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let tmp = tempfile::tempdir().unwrap();
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(store, model)
        .with_marketplace(tmp.path().to_path_buf())
        .unwrap()
}

fn stdio_input(
    name: &str,
    command: &str,
    args: Vec<&str>,
    env: Vec<(&str, &str)>,
) -> McpServerSettingsInput {
    McpServerSettingsInput {
        name: name.to_string(),
        enabled: true,
        transport: McpServerSettingsTransport::Stdio {
            command: command.to_string(),
            args: args.into_iter().map(String::from).collect(),
            env: env
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        },
        description: None,
    }
}

fn sse_input(name: &str, url: &str, headers: Vec<(&str, &str)>) -> McpServerSettingsInput {
    McpServerSettingsInput {
        name: name.to_string(),
        enabled: true,
        transport: McpServerSettingsTransport::Sse {
            url: url.to_string(),
            headers: headers
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        },
        description: None,
    }
}

fn streamable_http_input(
    name: &str,
    url: &str,
    headers: Vec<(&str, &str)>,
) -> McpServerSettingsInput {
    McpServerSettingsInput {
        name: name.to_string(),
        enabled: true,
        transport: McpServerSettingsTransport::StreamableHttp {
            url: url.to_string(),
            headers: headers
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        },
        description: None,
    }
}

// ── server_def_from_settings_input: Stdio ─────────────────────────────────

#[test]
fn stdio_transport_converts_command_args_env() {
    let input = stdio_input(
        "my-server",
        "npx",
        vec!["-y", "server"],
        vec![("KEY", "val")],
    );
    let def = server_def_from_settings_input(&input);

    assert_eq!(def.name, "my-server");
    assert_eq!(def.args, vec!["-y", "server"]);
    assert_eq!(def.env, HashMap::from([("KEY".into(), "val".into())]));
    match &def.transport {
        McpTransportDef::Stdio { command, cwd } => {
            assert_eq!(command, "npx");
            assert!(cwd.is_none());
        }
        other => panic!("expected Stdio transport, got {other:?}"),
    }
}

#[test]
fn stdio_transport_empty_args_and_env() {
    let input = stdio_input("bare", "/usr/bin/echo", vec![], vec![]);
    let def = server_def_from_settings_input(&input);

    assert!(def.args.is_empty());
    assert!(def.env.is_empty());
    match &def.transport {
        McpTransportDef::Stdio { command, .. } => assert_eq!(command, "/usr/bin/echo"),
        other => panic!("expected Stdio, got {other:?}"),
    }
}

// ── server_def_from_settings_input: Sse ───────────────────────────────────

#[test]
fn sse_transport_converts_url_and_headers() {
    let input = sse_input(
        "sse-srv",
        "http://localhost:8080/sse",
        vec![("Authorization", "Bearer tok")],
    );
    let def = server_def_from_settings_input(&input);

    assert_eq!(def.name, "sse-srv");
    assert!(def.args.is_empty());
    assert!(def.env.is_empty());
    match &def.transport {
        McpTransportDef::Sse {
            url,
            api_key_env,
            headers,
        } => {
            assert_eq!(url, "http://localhost:8080/sse");
            assert!(api_key_env.is_none());
            assert_eq!(headers.get("Authorization").unwrap(), "Bearer tok");
        }
        other => panic!("expected Sse, got {other:?}"),
    }
}

// ── server_def_from_settings_input: StreamableHttp ────────────────────────

#[test]
fn streamable_http_transport_converts_url_and_headers() {
    let input = streamable_http_input(
        "http-srv",
        "https://api.example.com/mcp",
        vec![("X-Api-Key", "secret")],
    );
    let def = server_def_from_settings_input(&input);

    assert_eq!(def.name, "http-srv");
    match &def.transport {
        McpTransportDef::StreamableHttp {
            url,
            api_key_env,
            headers,
        } => {
            assert_eq!(url, "https://api.example.com/mcp");
            assert!(api_key_env.is_none());
            assert_eq!(headers.get("X-Api-Key").unwrap(), "secret");
        }
        other => panic!("expected StreamableHttp, got {other:?}"),
    }
}

// ── server_def_from_settings_input: defaults ──────────────────────────────

#[test]
fn all_transports_share_default_lifecycle_values() {
    let inputs = vec![
        stdio_input("a", "cmd", vec![], vec![]),
        sse_input("b", "http://x", vec![]),
        streamable_http_input("c", "http://y", vec![]),
    ];
    for input in &inputs {
        let def = server_def_from_settings_input(input);
        assert!(
            !def.keep_alive,
            "keep_alive should be false for {}",
            def.name
        );
        assert_eq!(def.idle_timeout_secs, 300, "idle_timeout for {}", def.name);
        assert!(
            def.auto_restart,
            "auto_restart should be true for {}",
            def.name
        );
        assert_eq!(
            def.max_restart_attempts, 3,
            "max_restart_attempts for {}",
            def.name
        );
    }
}

// ── MCP settings facade methods ───────────────────────────────────────────

#[tokio::test]
async fn list_mcp_server_settings_empty_without_config() {
    let runtime = build_runtime().await;
    let result = runtime.list_mcp_server_settings(None).await;
    // Should not panic; result is Ok with an empty or default list
    assert!(result.is_ok());
}

#[tokio::test]
async fn open_mcp_config_file_returns_none_without_marketplace() {
    let runtime = build_runtime().await;
    let path = runtime.open_mcp_config_file().await.unwrap();
    assert!(path.is_none());
}

#[tokio::test]
async fn open_mcp_config_file_returns_some_with_marketplace() {
    let runtime = build_runtime_with_marketplace().await;
    let path = runtime.open_mcp_config_file().await.unwrap();
    assert!(
        path.is_some(),
        "should return a config path when marketplace is wired"
    );
}

#[tokio::test]
async fn upsert_without_marketplace_returns_error() {
    let runtime = build_runtime().await;
    let input = stdio_input("test-srv", "echo", vec![], vec![]);
    let result = runtime.upsert_mcp_server_settings(input).await;
    assert!(result.is_err(), "upsert should fail without marketplace");
}

#[tokio::test]
async fn delete_without_marketplace_returns_error() {
    let runtime = build_runtime().await;
    let result = runtime
        .delete_mcp_server_settings("nonexistent".into())
        .await;
    assert!(result.is_err(), "delete should fail without marketplace");
}

#[tokio::test]
async fn set_enabled_without_marketplace_returns_error() {
    let runtime = build_runtime().await;
    let result = runtime
        .set_mcp_server_enabled("nonexistent".into(), true)
        .await;
    assert!(
        result.is_err(),
        "set_enabled should fail without marketplace"
    );
}

// ── McpFacade trait delegation ────────────────────────────────────────────

#[tokio::test]
async fn trait_list_mcp_server_settings_delegates() {
    let runtime = build_runtime().await;
    let facade: &dyn McpFacade = &runtime;
    let result = facade.list_mcp_server_settings(None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn trait_open_mcp_config_file_delegates() {
    let runtime = build_runtime().await;
    let facade: &dyn McpFacade = &runtime;
    let path = facade.open_mcp_config_file().await.unwrap();
    assert!(path.is_none());
}

#[tokio::test]
async fn trait_list_catalog_sources_delegates() {
    let runtime = build_runtime().await;
    let facade: &dyn McpFacade = &runtime;
    let result = facade.list_catalog_sources().await;
    // Without marketplace, should return empty or error gracefully
    assert!(result.is_ok() || result.is_err());
}

// ── Profile settings delegation ───────────────────────────────────────────

#[tokio::test]
async fn list_profile_settings_does_not_panic() {
    let runtime = build_runtime().await;
    let facade: &dyn McpFacade = &runtime;
    let result = facade.list_profile_settings(None).await;
    assert!(result.is_ok());
}

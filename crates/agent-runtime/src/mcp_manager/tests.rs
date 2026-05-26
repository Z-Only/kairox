use super::*;
use agent_mcp::types::{McpServerDef, McpServerStatus, McpTransportDef};
use agent_mcp::McpError;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use std::collections::HashMap;

fn make_test_def(name: &str, keep_alive: bool) -> McpServerDef {
    McpServerDef {
        name: name.to_string(),
        transport: McpTransportDef::Stdio {
            command: "echo".to_string(),
            cwd: None,
        },
        args: vec![],
        env: HashMap::new(),
        keep_alive,
        idle_timeout_secs: 300,
        auto_restart: false,
        max_restart_attempts: 0,
    }
}

fn make_manager(configs: Vec<McpServerDef>) -> McpServerManager {
    let (event_tx, _) = tokio::sync::broadcast::channel(64);
    McpServerManager::from_config(
        configs,
        Arc::new(Mutex::new(ToolRegistry::new())),
        Arc::new(Mutex::new(PermissionEngine::new(
            ApprovalPolicy::Always,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        ))),
        Some(event_tx),
    )
}

#[test]
fn from_config_creates_all_servers_stopped() {
    let configs = vec![make_test_def("srv-a", false), make_test_def("srv-b", true)];
    let manager = make_manager(configs);
    assert_eq!(manager.server_count(), 2);
    let statuses = manager.server_statuses();
    assert_eq!(*statuses.get("srv-a").unwrap(), McpServerStatus::Stopped);
    assert_eq!(*statuses.get("srv-b").unwrap(), McpServerStatus::Stopped);
}

#[test]
fn server_statuses_returns_all_servers() {
    let configs = vec![make_test_def("alpha", false), make_test_def("beta", true)];
    let manager = make_manager(configs);
    let statuses = manager.server_statuses();
    assert_eq!(statuses.len(), 2);
    assert!(statuses.contains_key("alpha"));
    assert!(statuses.contains_key("beta"));
}

#[tokio::test]
async fn trust_and_revoke_server() {
    let manager = make_manager(vec![make_test_def("trusted-srv", false)]);
    assert!(!manager.is_trusted("trusted-srv").await);

    manager.trust_server("trusted-srv").await.unwrap();
    assert!(manager.is_trusted("trusted-srv").await);

    manager.revoke_trust("trusted-srv").await.unwrap();
    assert!(!manager.is_trusted("trusted-srv").await);
}

#[tokio::test]
async fn trust_unknown_server_is_noop() {
    let manager = make_manager(vec![]);
    // Trusting a server that's not in the manager — permission engine still records it
    manager.trust_server("unknown").await.unwrap();
    assert!(manager.is_trusted("unknown").await);
}

#[tokio::test]
async fn shutdown_all_on_empty_is_ok() {
    let mut manager = make_manager(vec![]);
    manager.shutdown_all().await.unwrap();
}

#[tokio::test]
async fn shutdown_all_stops_all_servers() {
    let configs = vec![make_test_def("srv-1", false), make_test_def("srv-2", false)];
    let mut manager = make_manager(configs);
    manager.shutdown_all().await.unwrap();
    let statuses = manager.server_statuses();
    for status in statuses.values() {
        assert_eq!(*status, McpServerStatus::Stopped);
    }
}

#[tokio::test]
async fn ensure_server_unknown_returns_error() {
    let mut manager = make_manager(vec![]);
    let result = manager.ensure_server("nonexistent").await;
    let err = result.err().unwrap();
    match err {
        McpError::NotRunning(name) => assert_eq!(name, "nonexistent"),
        other => panic!("expected NotRunning, got: {}", other),
    }
}

#[test]
fn server_def_returns_definition() {
    let configs = vec![make_test_def("my-server", true)];
    let manager = make_manager(configs);
    let def = manager.server_def("my-server").unwrap();
    assert_eq!(def.name, "my-server");
    assert!(def.keep_alive);
}

#[test]
fn server_def_unknown_returns_none() {
    let manager = make_manager(vec![]);
    assert!(manager.server_def("unknown").is_none());
}

#[tokio::test]
async fn register_dynamic_adds_server() {
    let mut m = make_manager(vec![]);
    assert!(!m.is_registered("alpha"));
    m.register_dynamic(make_test_def("alpha", false))
        .expect("register");
    assert!(m.is_registered("alpha"));
    assert_eq!(m.server_count(), 1);
}

#[tokio::test]
async fn register_dynamic_rejects_duplicate() {
    let mut m = make_manager(vec![]);
    m.register_dynamic(make_test_def("alpha", false)).unwrap();
    let err = m
        .register_dynamic(make_test_def("alpha", false))
        .unwrap_err();
    assert!(matches!(err, McpError::Protocol(msg) if msg.contains("already registered")));
}

#[tokio::test]
async fn unregister_dynamic_removes_server() {
    let mut m = make_manager(vec![make_test_def("alpha", false)]);
    assert!(m.is_registered("alpha"));
    m.unregister_dynamic("alpha").await.unwrap();
    assert!(!m.is_registered("alpha"));
}

#[tokio::test]
async fn unregister_dynamic_unknown_is_noop() {
    let mut m = make_manager(vec![]);
    m.unregister_dynamic("does-not-exist").await.unwrap();
}

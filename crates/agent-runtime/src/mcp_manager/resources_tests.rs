use super::*;
use agent_mcp::types::{McpServerDef, McpTransportDef};
use agent_tools::permission::PermissionEngine;
use agent_tools::registry::ToolRegistry;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

fn empty_manager() -> McpServerManager {
    let registry = Arc::new(Mutex::new(ToolRegistry::new()));
    let engine = Arc::new(Mutex::new(PermissionEngine::new(
        ApprovalPolicy::Always,
        SandboxPolicy::ReadOnly,
    )));
    McpServerManager::from_config(Vec::new(), registry, engine, None)
}

fn manager_with_stopped_server(server_id: &str) -> McpServerManager {
    let registry = Arc::new(Mutex::new(ToolRegistry::new()));
    let engine = Arc::new(Mutex::new(PermissionEngine::new(
        ApprovalPolicy::Always,
        SandboxPolicy::ReadOnly,
    )));
    let def = McpServerDef {
        name: server_id.to_string(),
        transport: McpTransportDef::Stdio {
            command: "/bin/false".to_string(),
            cwd: None,
        },
        args: Vec::new(),
        env: HashMap::new(),
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: false,
        max_restart_attempts: 3,
    };
    McpServerManager::from_config(vec![def], registry, engine, None)
}

#[tokio::test]
async fn list_resources_returns_not_running_when_server_id_unknown() {
    let manager = empty_manager();
    let error = manager
        .list_resources("missing")
        .await
        .expect_err("unknown server should error");
    assert!(
        matches!(error, McpError::NotRunning(ref name) if name == "missing"),
        "got: {error:?}"
    );
}

#[tokio::test]
async fn list_prompts_returns_not_running_when_server_id_unknown() {
    let manager = empty_manager();
    let error = manager
        .list_prompts("missing")
        .await
        .expect_err("unknown server should error");
    assert!(
        matches!(error, McpError::NotRunning(ref name) if name == "missing"),
        "got: {error:?}"
    );
}

#[tokio::test]
async fn read_resource_returns_not_running_when_server_id_unknown() {
    let manager = empty_manager();
    let error = manager
        .read_resource("missing", "file:///any")
        .await
        .expect_err("unknown server should error");
    assert!(
        matches!(error, McpError::NotRunning(ref name) if name == "missing"),
        "got: {error:?}"
    );
}

#[tokio::test]
async fn list_resources_returns_not_running_when_known_server_has_no_client() {
    let manager = manager_with_stopped_server("filesystem");
    let error = manager
        .list_resources("filesystem")
        .await
        .expect_err("stopped server should error");
    assert!(
        matches!(error, McpError::NotRunning(ref name) if name == "filesystem"),
        "got: {error:?}"
    );
}

#[tokio::test]
async fn list_prompts_returns_not_running_when_known_server_has_no_client() {
    let manager = manager_with_stopped_server("filesystem");
    let error = manager
        .list_prompts("filesystem")
        .await
        .expect_err("stopped server should error");
    assert!(
        matches!(error, McpError::NotRunning(ref name) if name == "filesystem"),
        "got: {error:?}"
    );
}

#[tokio::test]
async fn read_resource_returns_not_running_when_known_server_has_no_client() {
    let manager = manager_with_stopped_server("filesystem");
    let error = manager
        .read_resource("filesystem", "file:///x")
        .await
        .expect_err("stopped server should error");
    assert!(
        matches!(error, McpError::NotRunning(ref name) if name == "filesystem"),
        "got: {error:?}"
    );
}

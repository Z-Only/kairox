//! Runtime MCP integration tests.
//!
//! Tests McpServerManager integration with the runtime facade
//! using FakeModelClient and in-memory SQLite.

use agent_core::{AppFacade, StartSessionRequest};
use agent_mcp::{McpServerDef, McpTransportDef};
use agent_memory::SqliteMemoryStore;
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use std::sync::Arc;

async fn create_simple_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Simple response".into()]);
    let mem_store: Arc<dyn agent_memory::MemoryStore> =
        Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());
    LocalRuntime::new(store, model)
        .with_permission_mode(PermissionMode::Suggest)
        .with_context_limit(100_000)
        .with_memory_store(mem_store)
}

async fn create_runtime_with_mcp() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Simple response".into()]);
    let mem_store: Arc<dyn agent_memory::MemoryStore> =
        Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());

    let configs = vec![McpServerDef {
        name: "test-echo".into(),
        transport: McpTransportDef::Stdio {
            command: "echo".into(),
            cwd: None,
        },
        args: vec![],
        env: Default::default(),
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: false,
        max_restart_attempts: 0,
    }];

    LocalRuntime::new(store, model)
        .with_permission_mode(PermissionMode::Suggest)
        .with_context_limit(100_000)
        .with_memory_store(mem_store)
        .with_mcp_servers(configs)
        .await
}

#[tokio::test]
async fn runtime_without_mcp_servers_works() {
    let runtime = create_simple_runtime().await;
    let ws = runtime
        .open_workspace("/tmp/test-mcp-basic".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();
    assert!(!session_id.to_string().is_empty());
}

#[tokio::test]
async fn runtime_mcp_manager_with_empty_config() {
    let runtime = create_simple_runtime().await;
    // No MCP servers configured → mcp_manager is None
    assert!(runtime.mcp_manager().is_none());
}

#[tokio::test]
async fn runtime_with_mcp_servers_has_manager() {
    let runtime = create_runtime_with_mcp().await;
    // The manager should exist since we provided servers
    assert!(runtime.mcp_manager().is_some());
}

#[tokio::test]
async fn mcp_manager_reports_server_statuses() {
    let runtime = create_runtime_with_mcp().await;
    let manager = runtime.mcp_manager().unwrap();

    let statuses = manager.lock().await.server_statuses();
    // The server hasn't been started yet, so it should be present
    assert!(statuses.contains_key("test-echo"));
}

#[tokio::test]
async fn mcp_manager_trust_and_revoke() {
    let runtime = create_runtime_with_mcp().await;
    let manager = runtime.mcp_manager().unwrap();

    // Initially not trusted
    assert!(!manager.lock().await.is_trusted("test-echo").await);

    // Trust the server
    manager
        .lock()
        .await
        .trust_server("test-echo")
        .await
        .unwrap();
    assert!(manager.lock().await.is_trusted("test-echo").await);

    // Revoke trust
    manager
        .lock()
        .await
        .revoke_trust("test-echo")
        .await
        .unwrap();
    assert!(!manager.lock().await.is_trusted("test-echo").await);
}

#[tokio::test]
async fn mcp_manager_server_count() {
    let runtime = create_runtime_with_mcp().await;
    let manager = runtime.mcp_manager().unwrap();
    assert_eq!(manager.lock().await.server_count(), 1);
}

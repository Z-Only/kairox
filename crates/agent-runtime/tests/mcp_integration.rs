//! Runtime MCP integration tests.
//!
//! Tests McpServerManager integration with the runtime facade
//! using FakeModelClient and in-memory SQLite.

use agent_core::facade::{McpServerSettingsInput, McpServerSettingsTransport};
use agent_core::{AppFacade, StartSessionRequest};
use agent_mcp::{McpServerDef, McpTransportDef};
use agent_memory::SqliteMemoryStore;
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use std::collections::BTreeMap;
use std::sync::Arc;

async fn create_simple_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Simple response".into()]);
    let mem_store: Arc<dyn agent_memory::MemoryStore> =
        Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());
    LocalRuntime::new(store, model)
        .with_approval_and_sandbox(
            ApprovalPolicy::Always,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )
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
        .with_approval_and_sandbox(
            ApprovalPolicy::Always,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )
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
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();
    assert!(!session_id.to_string().is_empty());
}

#[tokio::test]
async fn runtime_mcp_manager_with_empty_config() {
    let runtime = create_simple_runtime()
        .await
        .with_mcp_servers(Vec::new())
        .await;

    let manager = runtime
        .mcp_manager()
        .expect("empty MCP config should still create a manager for settings updates");
    assert_eq!(manager.lock().await.server_count(), 0);
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

#[tokio::test]
async fn upserting_enabled_mcp_server_registers_runtime_server() {
    let config_dir = tempfile::tempdir().expect("config dir");
    let runtime = create_simple_runtime()
        .await
        .with_marketplace_loaded(config_dir.path().to_path_buf(), &[])
        .expect("marketplace config")
        .with_mcp_servers(Vec::new())
        .await;

    runtime
        .upsert_mcp_server_settings(McpServerSettingsInput {
            name: "live-added".into(),
            transport: McpServerSettingsTransport::Stdio {
                command: "node".into(),
                args: vec!["echo-server.mjs".into()],
                env: BTreeMap::from([("ECHO_MODE".into(), "1".into())]),
            },
            enabled: true,
            description: Some("added from settings".into()),
        })
        .await
        .unwrap();

    let manager = runtime.mcp_manager().expect("MCP manager");
    let manager = manager.lock().await;
    assert!(manager.is_registered("live-added"));
    let def = manager.server_def("live-added").expect("server def");
    match &def.transport {
        McpTransportDef::Stdio { command, cwd } => {
            assert_eq!(command, "node");
            assert_eq!(cwd, &None);
        }
        other => panic!("expected stdio transport, got {other:?}"),
    }
    assert_eq!(def.args, vec!["echo-server.mjs"]);
    assert_eq!(def.env.get("ECHO_MODE").map(String::as_str), Some("1"));
}

#[tokio::test]
async fn upserting_existing_mcp_server_replaces_runtime_definition() {
    let config_dir = tempfile::tempdir().expect("config dir");
    let runtime = create_runtime_with_mcp()
        .await
        .with_marketplace_loaded(config_dir.path().to_path_buf(), &[])
        .expect("marketplace config");

    runtime
        .upsert_mcp_server_settings(McpServerSettingsInput {
            name: "test-echo".into(),
            transport: McpServerSettingsTransport::Stdio {
                command: "node".into(),
                args: vec!["replacement.mjs".into()],
                env: BTreeMap::new(),
            },
            enabled: true,
            description: None,
        })
        .await
        .unwrap();

    let manager = runtime.mcp_manager().expect("MCP manager");
    let manager = manager.lock().await;
    let def = manager.server_def("test-echo").expect("server def");
    match &def.transport {
        McpTransportDef::Stdio { command, cwd } => {
            assert_eq!(command, "node");
            assert_eq!(cwd, &None);
        }
        other => panic!("expected stdio transport, got {other:?}"),
    }
    assert_eq!(def.args, vec!["replacement.mjs"]);
    assert_eq!(manager.server_count(), 1);
}

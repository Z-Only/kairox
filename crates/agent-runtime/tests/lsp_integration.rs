//! Runtime LSP/DAP integration tests.
//!
//! Tests LspServerManager integration with the runtime facade
//! using FakeModelClient and in-memory SQLite.

use agent_lsp::{DapServerDef, LspServerDef};
use agent_memory::SqliteMemoryStore;
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use std::collections::HashMap;
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

async fn create_runtime_with_lsp() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Simple response".into()]);
    let mem_store: Arc<dyn agent_memory::MemoryStore> =
        Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());

    let lsp_configs = vec![LspServerDef {
        name: "test-lsp".into(),
        command: "echo".into(),
        args: vec![],
        env: HashMap::new(),
        cwd: None,
        languages: vec!["rust".into()],
        file_patterns: vec!["*.rs".into()],
        initialization_options: None,
    }];

    let dap_configs = vec![DapServerDef {
        name: "test-dap".into(),
        command: "echo".into(),
        args: vec![],
        env: HashMap::new(),
        cwd: None,
        languages: vec!["rust".into()],
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
        .with_lsp_servers(lsp_configs, dap_configs)
        .await
}

#[tokio::test]
async fn runtime_without_lsp_servers_works() {
    let runtime = create_simple_runtime().await;
    assert!(runtime.lsp_manager().is_none());
}

#[tokio::test]
async fn runtime_with_lsp_servers_has_manager() {
    let runtime = create_runtime_with_lsp().await;
    assert!(runtime.lsp_manager().is_some());
}

#[tokio::test]
async fn lsp_manager_reports_server_ids() {
    let runtime = create_runtime_with_lsp().await;
    let manager = runtime.lsp_manager().unwrap();

    let lsp_ids = manager.lock().await.lsp_server_ids();
    assert!(lsp_ids.contains(&"test-lsp".to_string()));

    let dap_ids = manager.lock().await.dap_server_ids();
    assert!(dap_ids.contains(&"test-dap".to_string()));
}

#[tokio::test]
async fn lsp_manager_empty_config_returns_none() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Simple response".into()]);
    let mem_store: Arc<dyn agent_memory::MemoryStore> =
        Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());

    let runtime = LocalRuntime::new(store, model)
        .with_approval_and_sandbox(
            ApprovalPolicy::Always,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )
        .with_context_limit(100_000)
        .with_memory_store(mem_store)
        .with_lsp_servers(vec![], vec![])
        .await;

    // Empty config → no manager created
    assert!(runtime.lsp_manager().is_none());
}

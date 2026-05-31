//! Runtime LSP/DAP integration tests.
//!
//! Tests LspServerManager integration with the runtime facade
//! using FakeModelClient and in-memory SQLite.

use agent_core::AppFacade;
use agent_lsp::{DapServerDef, LspServerDef};
use agent_memory::SqliteMemoryStore;
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use std::collections::HashMap;
use std::sync::Arc;

const MOCK_STDIO_SERVER: &str = r#"
import json
import os
import sys

def read_message():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        line = line.decode("ascii").strip()
        if not line:
            break
        key, value = line.split(":", 1)
        headers[key.lower()] = value.strip()
    length = int(headers["content-length"])
    return json.loads(sys.stdin.buffer.read(length))

def write_message(payload):
    data = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    sys.stdout.buffer.write(f"Content-Length: {len(data)}\r\n\r\n".encode("ascii"))
    sys.stdout.buffer.write(data)
    sys.stdout.buffer.flush()

while True:
    msg = read_message()
    if msg is None:
        break
    if "id" not in msg:
        continue
    params = msg.get("params") or {}
    if msg.get("method") == "initialize" and "rootUri" in params:
        log_path = os.environ.get("MOCK_LSP_LOG")
        if log_path:
            with open(log_path, "a", encoding="utf-8") as handle:
                handle.write(params["rootUri"] + "\n")
        result = {"capabilities": {"hoverProvider": True, "definitionProvider": True}}
    else:
        dap_request = params if isinstance(params, dict) else {}
        result = {
            "seq": 1,
            "type": "response",
            "request_seq": dap_request.get("seq", 1),
            "command": msg.get("method", "unknown"),
            "success": True,
            "body": {}
        }
    write_message({"jsonrpc": "2.0", "id": msg["id"], "result": result})
"#;

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

async fn create_runtime_with_mock_lsp(
    log_path: &std::path::Path,
) -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let mut env = HashMap::new();
    env.insert("MOCK_LSP_LOG".to_string(), log_path.display().to_string());
    let lsp_configs = vec![LspServerDef {
        name: "mock-lsp".into(),
        command: "python3".into(),
        args: vec!["-u".into(), "-c".into(), MOCK_STDIO_SERVER.into()],
        env: env.clone(),
        cwd: None,
        languages: vec!["rust".into()],
        file_patterns: vec!["*.rs".into()],
        initialization_options: None,
    }];
    let dap_configs = vec![DapServerDef {
        name: "mock-dap".into(),
        command: "python3".into(),
        args: vec!["-u".into(), "-c".into(), MOCK_STDIO_SERVER.into()],
        env,
        cwd: None,
        languages: vec!["rust".into()],
    }];

    create_simple_runtime()
        .await
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

#[tokio::test]
async fn opening_workspace_registers_lsp_and_dap_tools() {
    let temp = tempfile::tempdir().unwrap();
    let log_path = temp.path().join("lsp-roots.log");
    let workspace = temp.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let runtime = create_runtime_with_mock_lsp(&log_path).await;

    runtime
        .open_workspace(workspace.display().to_string())
        .await
        .unwrap();

    let tools = runtime.tool_registry().lock().await.list_all().await;
    let ids: Vec<_> = tools.into_iter().map(|tool| tool.tool_id).collect();
    assert!(ids.contains(&"lsp.mock-lsp.hover".to_string()));
    assert!(ids.contains(&"debug.mock-dap.launch".to_string()));
}

#[tokio::test]
async fn project_draft_session_restarts_lsp_for_project_root() {
    let temp = tempfile::tempdir().unwrap();
    let log_path = temp.path().join("lsp-roots.log");
    let workspace = temp.path().join("launch-workspace");
    let project_root = temp.path().join("project-root");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    tokio::fs::create_dir_all(&project_root).await.unwrap();
    let runtime = create_runtime_with_mock_lsp(&log_path).await;

    let workspace = runtime
        .open_workspace(workspace.display().to_string())
        .await
        .unwrap();
    let project = runtime
        .add_existing_project(workspace.workspace_id, project_root.display().to_string())
        .await
        .unwrap();

    runtime
        .create_project_draft_session(project.project_id)
        .await
        .unwrap();

    let roots = std::fs::read_to_string(log_path).unwrap();
    assert!(
        roots
            .lines()
            .any(|line| line == format!("file://{}", project_root.display())),
        "expected LSP to initialize for project root, got:\n{roots}"
    );
}

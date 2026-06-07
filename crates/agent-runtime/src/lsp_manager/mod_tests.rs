use super::*;
use agent_tools::permission::PermissionEngine;
use agent_tools::registry::ToolRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn empty_manager() -> LspServerManager {
    let tool_registry = Arc::new(Mutex::new(ToolRegistry::new()));
    let permission_engine = Arc::new(Mutex::new(PermissionEngine::default()));
    LspServerManager::from_config(vec![], vec![], tool_registry, permission_engine, None)
}

fn sample_lsp_def(name: &str) -> LspServerDef {
    LspServerDef {
        name: name.to_string(),
        command: "echo".to_string(),
        args: vec![],
        env: HashMap::new(),
        cwd: None,
        languages: vec!["rust".to_string()],
        file_patterns: vec!["*.rs".to_string()],
        initialization_options: None,
    }
}

fn sample_dap_def(name: &str) -> DapServerDef {
    DapServerDef {
        name: name.to_string(),
        command: "echo".to_string(),
        args: vec![],
        env: HashMap::new(),
        cwd: None,
        languages: vec!["rust".to_string()],
    }
}

// ---------------------------------------------------------------------------
// file_uri_from_path tests
// ---------------------------------------------------------------------------

#[test]
fn file_uri_from_path_percent_encodes_spaces() {
    assert_eq!(
        file_uri_from_path("/tmp/kairox project"),
        "file:///tmp/kairox%20project"
    );
}

#[test]
fn file_uri_from_path_absolute_path() {
    assert_eq!(file_uri_from_path("/tmp/test"), "file:///tmp/test");
}

#[test]
fn file_uri_from_path_root() {
    assert_eq!(file_uri_from_path("/"), "file:///");
}

#[test]
fn file_uri_from_path_unicode() {
    let uri = file_uri_from_path("/tmp/测试目录/文件.rs");
    assert!(uri.starts_with("file:///tmp/"));
    // Url encodes each UTF-8 byte as %XX
    assert!(uri.contains("%E6%B5%8B%E8%AF%95") || uri.contains("测试"));
}

#[test]
fn file_uri_from_path_special_chars() {
    let uri = file_uri_from_path("/tmp/a#b?c%d");
    assert!(uri.starts_with("file:///tmp/"));
    // '#', '?', '%' must be percent-encoded in a valid URI
    assert!(uri.contains("%23"), "expected '#' to be encoded: {uri}");
    assert!(uri.contains("%3F"), "expected '?' to be encoded: {uri}");
    assert!(uri.contains("%25"), "expected '%' to be encoded: {uri}");
}

// ---------------------------------------------------------------------------
// LspServerManager tests
// ---------------------------------------------------------------------------

#[test]
fn lsp_server_manager_from_config_empty() {
    let manager = empty_manager();
    assert!(manager.lsp_server_ids().is_empty());
    assert!(manager.dap_server_ids().is_empty());
}

#[test]
fn lsp_server_manager_from_config_populates_ids() {
    let tool_registry = Arc::new(Mutex::new(ToolRegistry::new()));
    let permission_engine = Arc::new(Mutex::new(PermissionEngine::default()));
    let manager = LspServerManager::from_config(
        vec![sample_lsp_def("rust-analyzer"), sample_lsp_def("pyright")],
        vec![sample_dap_def("codelldb")],
        tool_registry,
        permission_engine,
        None,
    );

    let mut lsp_ids = manager.lsp_server_ids();
    lsp_ids.sort();
    assert_eq!(lsp_ids, vec!["pyright", "rust-analyzer"]);

    let dap_ids = manager.dap_server_ids();
    assert_eq!(dap_ids, vec!["codelldb"]);
}

#[tokio::test]
async fn lsp_server_manager_start_unknown_lsp_returns_error() {
    let mut manager = empty_manager();
    let result = manager.start_lsp_server("nonexistent", "file:///tmp").await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("nonexistent"),
        "error should mention the server id: {error_msg}"
    );
}

#[tokio::test]
async fn lsp_server_manager_start_unknown_dap_returns_error() {
    let mut manager = empty_manager();
    let result = manager.start_dap_server("nonexistent").await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("nonexistent"),
        "error should mention the server id: {error_msg}"
    );
}

#[tokio::test]
async fn lsp_server_manager_stop_unknown_lsp_returns_error() {
    let mut manager = empty_manager();
    let result = manager.stop_lsp_server("nonexistent").await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("nonexistent"),
        "error should mention the server id: {error_msg}"
    );
}

#[tokio::test]
async fn lsp_server_manager_stop_unknown_dap_returns_error() {
    let mut manager = empty_manager();
    let result = manager.stop_dap_server("nonexistent").await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("nonexistent"),
        "error should mention the server id: {error_msg}"
    );
}

#[tokio::test]
async fn lsp_server_manager_shutdown_all_empty_is_ok() {
    let mut manager = empty_manager();
    let result = manager.shutdown_all().await;
    assert!(result.is_ok());
}

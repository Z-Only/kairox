use std::collections::HashMap;

use super::*;
use crate::types::ServerStatus;

// ── LspServerDef construction ──

fn sample_lsp_def() -> LspServerDef {
    LspServerDef {
        name: "rust-analyzer".to_string(),
        command: "rust-analyzer".to_string(),
        args: vec!["--stdio".to_string()],
        env: HashMap::from([("RUST_LOG".to_string(), "info".to_string())]),
        cwd: Some("/tmp/project".to_string()),
        languages: vec!["rust".to_string()],
        file_patterns: vec!["*.rs".to_string()],
        initialization_options: Some(serde_json::json!({"checkOnSave": true})),
    }
}

fn sample_dap_def() -> DapServerDef {
    DapServerDef {
        name: "debugpy".to_string(),
        command: "python".to_string(),
        args: vec!["-m".to_string(), "debugpy.adapter".to_string()],
        env: HashMap::new(),
        cwd: None,
        languages: vec!["python".to_string()],
    }
}

#[test]
fn lsp_server_def_stores_all_fields() {
    let def = sample_lsp_def();
    assert_eq!(def.name, "rust-analyzer");
    assert_eq!(def.command, "rust-analyzer");
    assert_eq!(def.args, vec!["--stdio"]);
    assert_eq!(def.env.get("RUST_LOG").unwrap(), "info");
    assert_eq!(def.cwd.as_deref(), Some("/tmp/project"));
    assert_eq!(def.languages, vec!["rust"]);
    assert_eq!(def.file_patterns, vec!["*.rs"]);
    assert!(def.initialization_options.is_some());
}

#[test]
fn lsp_server_def_clone() {
    let def = sample_lsp_def();
    let cloned = def.clone();
    assert_eq!(def.name, cloned.name);
    assert_eq!(def.command, cloned.command);
    assert_eq!(def.args, cloned.args);
}

#[test]
fn lsp_server_def_debug() {
    let def = sample_lsp_def();
    let dbg = format!("{:?}", def);
    assert!(dbg.contains("rust-analyzer"));
}

#[test]
fn lsp_server_def_no_optional_fields() {
    let def = LspServerDef {
        name: "minimal".to_string(),
        command: "lsp-server".to_string(),
        args: vec![],
        env: HashMap::new(),
        cwd: None,
        languages: vec![],
        file_patterns: vec![],
        initialization_options: None,
    };
    assert!(def.cwd.is_none());
    assert!(def.initialization_options.is_none());
    assert!(def.args.is_empty());
    assert!(def.languages.is_empty());
}

// ── DapServerDef construction ──

#[test]
fn dap_server_def_stores_all_fields() {
    let def = sample_dap_def();
    assert_eq!(def.name, "debugpy");
    assert_eq!(def.command, "python");
    assert_eq!(def.args.len(), 2);
    assert!(def.env.is_empty());
    assert!(def.cwd.is_none());
    assert_eq!(def.languages, vec!["python"]);
}

#[test]
fn dap_server_def_clone() {
    let def = sample_dap_def();
    let cloned = def.clone();
    assert_eq!(def.name, cloned.name);
    assert_eq!(def.languages, cloned.languages);
}

#[test]
fn dap_server_def_debug() {
    let def = sample_dap_def();
    let dbg = format!("{:?}", def);
    assert!(dbg.contains("debugpy"));
}

// ── LspServerLifecycle ──

#[test]
fn lsp_lifecycle_new_starts_stopped() {
    let lc = LspServerLifecycle::new(sample_lsp_def());
    assert_eq!(*lc.status(), ServerStatus::Stopped);
    assert!(lc.client().is_none());
}

#[test]
fn lsp_lifecycle_exposes_def() {
    let lc = LspServerLifecycle::new(sample_lsp_def());
    assert_eq!(lc.def.name, "rust-analyzer");
    assert_eq!(lc.def.command, "rust-analyzer");
}

#[tokio::test]
async fn lsp_lifecycle_stop_when_already_stopped() {
    let mut lc = LspServerLifecycle::new(sample_lsp_def());
    // Stopping an already-stopped lifecycle should succeed.
    lc.stop().await.unwrap();
    assert_eq!(*lc.status(), ServerStatus::Stopped);
    assert!(lc.client().is_none());
}

// ── DapServerLifecycle ──

#[test]
fn dap_lifecycle_new_starts_stopped() {
    let lc = DapServerLifecycle::new(sample_dap_def());
    assert_eq!(*lc.status(), ServerStatus::Stopped);
    assert!(lc.client().is_none());
}

#[test]
fn dap_lifecycle_exposes_def() {
    let lc = DapServerLifecycle::new(sample_dap_def());
    assert_eq!(lc.def.name, "debugpy");
    assert_eq!(lc.def.command, "python");
}

#[tokio::test]
async fn dap_lifecycle_stop_when_already_stopped() {
    let mut lc = DapServerLifecycle::new(sample_dap_def());
    // Stopping an already-stopped lifecycle should succeed.
    lc.stop().await.unwrap();
    assert_eq!(*lc.status(), ServerStatus::Stopped);
    assert!(lc.client().is_none());
}

//! Connectivity test for a deterministic local stdio MCP server.
//!
//! This avoids environment-specific remote servers and proxy binaries while
//! still exercising the same ServerLifecycle connectivity path.

use agent_mcp::types::{ConnectivityResult, McpServerDef, McpTransportDef};
use agent_mcp::ServerLifecycle;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::time::Duration;

fn node_available() -> bool {
    let available = Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    if !available {
        eprintln!("skipping: node not available on PATH");
    }

    available
}

fn minimal_server_def() -> McpServerDef {
    McpServerDef {
        name: "minimal-connectivity".into(),
        transport: McpTransportDef::Stdio {
            command: "node".into(),
            cwd: None,
        },
        args: vec!["tests/fixtures/minimal-mcp-server.mjs".into()],
        env: HashMap::new(),
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: false,
        max_restart_attempts: 1,
    }
}

#[tokio::test]
async fn stdio_fixture_connectivity() {
    if !node_available() {
        return;
    }

    let mut lifecycle = ServerLifecycle::new(minimal_server_def());
    let result = lifecycle
        .test_connectivity(Some(Duration::from_secs(5)))
        .await;
    lifecycle.shutdown().await.ok();

    match result {
        ConnectivityResult::Connected { tool_count } => {
            assert!(
                tool_count > 0,
                "expected minimal fixture tools to be discovered, got {tool_count}"
            );
        }
        ConnectivityResult::Failed { reason } => {
            panic!("stdio MCP fixture connectivity failed: {reason}");
        }
    }
}

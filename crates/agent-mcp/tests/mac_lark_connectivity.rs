//! Connectivity test for the mac-lark MCP server via utoo-proxy.
//!
//! The mac-lark server is a remote Streamable-HTTP MCP server that provides
//! 语雀 (Yuque) integration. `utoo-proxy` acts as a local stdio-to-HTTP bridge.
//!
//! This test is skipped when `utoo-proxy` is not on PATH.

use agent_mcp::types::{ConnectivityResult, McpServerDef, McpTransportDef};
use agent_mcp::ServerLifecycle;

fn utoo_proxy_available() -> bool {
    std::process::Command::new("utoo-proxy")
        .arg("--help")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn mac_lark_connectivity() {
    if !utoo_proxy_available() {
        eprintln!("SKIP: utoo-proxy not found on PATH");
        return;
    }

    let def = McpServerDef {
        name: "mac-lark".into(),
        transport: McpTransportDef::Stdio {
            command: "utoo-proxy".into(),
            cwd: None,
        },
        args: vec![
            "https://mcpgwoffice-prod.alipay.com/mcpgw/v1/shttpproxy/message/MAIN_CHAIR_mcp.ant.faas.skylarkmcpserver.skylarkmcpserver".into(),
            "-t".into(),
            "STREAMABLE_HTTP".into(),
            "-l".into(),
            "info".into(),
        ],
        env: Default::default(),
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: false,
        max_restart_attempts: 1,
    };

    let mut lifecycle = ServerLifecycle::new(def);
    let result = lifecycle
        .test_connectivity(Some(std::time::Duration::from_secs(15)))
        .await;

    match result {
        ConnectivityResult::Connected { tool_count } => {
            println!("SUCCESS: mac-lark connected with {} tools", tool_count);
            assert!(
                tool_count > 0,
                "expected at least 1 tool, got {}",
                tool_count
            );
        }
        ConnectivityResult::Failed { reason } => {
            panic!("mac-lark connectivity FAILED: {}", reason);
        }
    }
}

//! Lifecycle integration tests using the echo-mcp-server fixture.
//!
//! These tests exercise the full ServerLifecycle state machine with a real subprocess.

use agent_mcp::lifecycle::ServerLifecycle;
use agent_mcp::types::{McpServerDef, McpServerStatus, McpTransportDef};
use std::collections::HashMap;

/// Check whether `node` and the fixture dependencies are available.
fn echo_fixture_available() -> bool {
    let available = std::process::Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg("await import('@modelcontextprotocol/sdk/server/mcp.js'); await import('zod');")
        .current_dir("tests/fixtures")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !available {
        eprintln!("skipping: node fixture dependencies not available; run bun install --no-save in crates/agent-mcp/tests/fixtures to enable these tests");
    }
    available
}

/// Create a stdio McpServerDef pointing at the echo-mcp-server fixture.
fn echo_server_def() -> McpServerDef {
    McpServerDef {
        name: "echo-test".into(),
        transport: McpTransportDef::Stdio {
            command: "node".into(),
            cwd: None,
        },
        args: vec!["tests/fixtures/echo-mcp-server.mjs".into()],
        env: HashMap::new(),
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: true,
        max_restart_attempts: 3,
    }
}

#[tokio::test]
async fn full_lifecycle_start_discover_call() {
    if !echo_fixture_available() {
        return;
    }
    let mut lifecycle = ServerLifecycle::new(echo_server_def());

    // Start the server
    let client = lifecycle
        .ensure_running()
        .await
        .expect("ensure_running failed");
    assert_eq!(*lifecycle.status(), McpServerStatus::Running);

    // Discover tools
    let tools = client
        .discover_tools()
        .await
        .expect("discover_tools failed");
    assert!(!tools.is_empty(), "Should discover at least one tool");

    // Call a tool
    let result = client
        .call_tool("echo", serde_json::json!({"message": "hello"}))
        .await
        .expect("call_tool failed");
    assert!(!result.content.is_empty());

    // Mark active
    lifecycle.mark_active();
    assert!(lifecycle.last_activity().is_some());

    // Shutdown
    lifecycle.shutdown().await.expect("shutdown failed");
    assert_eq!(*lifecycle.status(), McpServerStatus::Stopped);
}

#[tokio::test]
async fn keep_alive_server_never_times_out() {
    if !echo_fixture_available() {
        return;
    }
    let mut def = echo_server_def();
    def.keep_alive = true;
    def.idle_timeout_secs = 1;

    let mut lifecycle = ServerLifecycle::new(def);
    lifecycle
        .ensure_running()
        .await
        .expect("ensure_running failed");
    lifecycle.mark_active();

    // Wait longer than the idle timeout
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    // Should NOT shut down because keep_alive = true
    lifecycle
        .check_idle_timeout()
        .await
        .expect("check_idle_timeout failed");
    assert_eq!(*lifecycle.status(), McpServerStatus::Running);

    lifecycle.shutdown().await.ok();
}

#[tokio::test]
async fn idle_timeout_shuts_down_server() {
    if !echo_fixture_available() {
        return;
    }
    let mut def = echo_server_def();
    def.keep_alive = false;
    def.idle_timeout_secs = 1;

    let mut lifecycle = ServerLifecycle::new(def);
    lifecycle
        .ensure_running()
        .await
        .expect("ensure_running failed");
    lifecycle.mark_active();

    // Wait longer than the idle timeout
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    // Should shut down
    lifecycle
        .check_idle_timeout()
        .await
        .expect("check_idle_timeout failed");
    assert_eq!(*lifecycle.status(), McpServerStatus::Stopped);
}

#[tokio::test]
async fn restart_after_shutdown() {
    if !echo_fixture_available() {
        return;
    }
    let mut lifecycle = ServerLifecycle::new(echo_server_def());

    // Start, then shut down
    lifecycle
        .ensure_running()
        .await
        .expect("first ensure_running failed");
    assert_eq!(*lifecycle.status(), McpServerStatus::Running);
    lifecycle.shutdown().await.expect("shutdown failed");
    assert_eq!(*lifecycle.status(), McpServerStatus::Stopped);

    // Restart — should work
    let client = lifecycle
        .ensure_running()
        .await
        .expect("second ensure_running failed");
    assert_eq!(*lifecycle.status(), McpServerStatus::Running);

    // Verify the restarted server works
    let tools = client
        .discover_tools()
        .await
        .expect("discover_tools after restart failed");
    assert!(!tools.is_empty());

    lifecycle.shutdown().await.ok();
}

#[tokio::test]
async fn discovery_cache_available_when_running() {
    if !echo_fixture_available() {
        return;
    }
    let mut lifecycle = ServerLifecycle::new(echo_server_def());

    // Before starting, no discovery cache
    assert!(lifecycle.discovery().is_none());

    // After starting, discovery cache is available
    lifecycle
        .ensure_running()
        .await
        .expect("ensure_running failed");
    assert!(lifecycle.discovery().is_some());

    // After shutdown, discovery cache is gone
    lifecycle.shutdown().await.ok();
    assert!(lifecycle.discovery().is_none());
}

use super::*;
use crate::types::{McpServerDef, McpServerStatus, McpTransportDef};
use std::collections::HashMap;

/// Helper: create a test `McpServerDef` with sensible defaults for stdio.
fn create_test_stdio_def() -> McpServerDef {
    McpServerDef {
        name: "test-server".into(),
        transport: McpTransportDef::Stdio {
            command: "cat".into(),
            cwd: None,
        },
        args: vec![],
        env: HashMap::new(),
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: false,
        max_restart_attempts: 1,
    }
}

#[tokio::test]
async fn check_connectivity_returns_failed_when_server_cannot_start() {
    // "cat" doesn't speak MCP, so ensure_running will fail.
    let mut lifecycle = ServerLifecycle::new(create_test_stdio_def());
    let result = check_connectivity(&mut lifecycle, Some(Duration::from_secs(5))).await;
    match result {
        ConnectivityResult::Failed { reason } => {
            assert!(
                reason.contains("failed to start server"),
                "unexpected reason: {reason}"
            );
        }
        ConnectivityResult::Connected { .. } => {
            panic!("expected Failed, got Connected");
        }
    }
    assert_eq!(*lifecycle.status(), McpServerStatus::Failed);
}

#[tokio::test]
async fn check_connectivity_uses_default_timeout() {
    // Ensure the function doesn't panic with None timeout.
    let mut lifecycle = ServerLifecycle::new(create_test_stdio_def());
    let result = check_connectivity(&mut lifecycle, None).await;
    assert!(matches!(result, ConnectivityResult::Failed { .. }));
}

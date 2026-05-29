use super::*;
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
        auto_restart: true,
        max_restart_attempts: 3,
    }
}

/// Helper: create a test `ServerLifecycle` that won't actually start a server.
fn create_test_lifecycle() -> ServerLifecycle {
    ServerLifecycle::new(create_test_stdio_def())
}

#[test]
fn new_server_is_stopped() {
    let lifecycle = ServerLifecycle::new(create_test_stdio_def());
    assert_eq!(*lifecycle.status(), McpServerStatus::Stopped);
    assert_eq!(lifecycle.server_id(), "test-server");
    assert!(lifecycle.client.is_none());
    assert!(lifecycle.discovery.is_none());
    assert!(lifecycle.last_activity.is_none());
    assert_eq!(lifecycle.restart_count(), 0);
}

#[test]
fn server_id_returns_def_name() {
    let mut def = create_test_stdio_def();
    def.name = "my-custom-id".into();
    let lifecycle = ServerLifecycle::new(def);
    assert_eq!(lifecycle.server_id(), "my-custom-id");
}

#[test]
fn mark_active_updates_last_activity() {
    let mut lifecycle = create_test_lifecycle();
    assert!(lifecycle.last_activity().is_none());
    lifecycle.mark_active();
    assert!(lifecycle.last_activity().is_some());
}

#[test]
fn mark_active_updates_timestamp_on_repeated_calls() {
    let mut lifecycle = create_test_lifecycle();
    lifecycle.mark_active();
    let first = lifecycle.last_activity().unwrap();
    // Small sleep to ensure time progresses
    std::thread::sleep(std::time::Duration::from_millis(10));
    lifecycle.mark_active();
    let second = lifecycle.last_activity().unwrap();
    assert!(second > first);
}

#[tokio::test]
async fn shutdown_on_stopped_is_noop() {
    let mut lifecycle = create_test_lifecycle();
    assert_eq!(*lifecycle.status(), McpServerStatus::Stopped);
    lifecycle.shutdown().await.unwrap();
    assert_eq!(*lifecycle.status(), McpServerStatus::Stopped);
}

#[tokio::test]
async fn shutdown_sets_status_to_stopped() {
    let mut lifecycle = create_test_lifecycle();
    // Manually set status to Running (without a real client) to test
    // that shutdown transitions it to Stopped.
    lifecycle.status = McpServerStatus::Running;
    lifecycle.shutdown().await.unwrap();
    assert_eq!(*lifecycle.status(), McpServerStatus::Stopped);
    assert!(lifecycle.client.is_none());
    assert!(lifecycle.discovery.is_none());
}

#[tokio::test]
async fn check_idle_timeout_does_nothing_if_no_activity() {
    let mut lifecycle = create_test_lifecycle();
    // No activity recorded → should not try to shut down.
    lifecycle.check_idle_timeout().await.unwrap();
    assert_eq!(*lifecycle.status(), McpServerStatus::Stopped);
}

#[tokio::test]
async fn check_idle_timeout_does_nothing_if_keep_alive() {
    let mut def = create_test_stdio_def();
    def.keep_alive = true;
    def.idle_timeout_secs = 0; // Would immediately time out if not keep_alive
    let mut lifecycle = ServerLifecycle::new(def);
    lifecycle.mark_active();
    lifecycle.status = McpServerStatus::Running;

    lifecycle.check_idle_timeout().await.unwrap();
    assert_eq!(*lifecycle.status(), McpServerStatus::Running);
}

#[tokio::test]
async fn check_idle_timeout_shuts_down_when_expired() {
    let mut def = create_test_stdio_def();
    def.keep_alive = false;
    def.idle_timeout_secs = 0; // Immediate timeout
    let mut lifecycle = ServerLifecycle::new(def);
    lifecycle.mark_active();
    lifecycle.status = McpServerStatus::Running;

    // Since idle_timeout_secs is 0 and some time has passed since mark_active,
    // the timeout should trigger shutdown.
    // But with 0, the duration check is elapsed > 0, which is almost certainly true.
    // Add a tiny sleep to ensure elapsed > 0.
    std::thread::sleep(std::time::Duration::from_millis(1));
    lifecycle.check_idle_timeout().await.unwrap();
    assert_eq!(*lifecycle.status(), McpServerStatus::Stopped);
}

#[tokio::test]
async fn check_idle_timeout_does_nothing_if_not_expired() {
    let mut def = create_test_stdio_def();
    def.keep_alive = false;
    def.idle_timeout_secs = 86400; // 24 hours — won't expire
    let mut lifecycle = ServerLifecycle::new(def);
    lifecycle.mark_active();
    lifecycle.status = McpServerStatus::Running;

    lifecycle.check_idle_timeout().await.unwrap();
    assert_eq!(*lifecycle.status(), McpServerStatus::Running);
}

#[tokio::test]
async fn ensure_running_with_cat_fails_handshake() {
    // "cat" is a long-running process but doesn't speak MCP protocol,
    // so the handshake should fail. This tests the failure → Failed path.
    let mut lifecycle = create_test_lifecycle();
    let result = lifecycle.ensure_running().await;
    assert!(result.is_err(), "cat doesn't speak MCP, should fail");
    assert_eq!(*lifecycle.status(), McpServerStatus::Failed);
}

#[tokio::test]
async fn ensure_running_no_auto_restart_gives_up_on_failure() {
    let mut def = create_test_stdio_def();
    def.auto_restart = false;
    def.max_restart_attempts = 0;
    let mut lifecycle = ServerLifecycle::new(def);
    let result = lifecycle.ensure_running().await;
    assert!(result.is_err());
    assert_eq!(*lifecycle.status(), McpServerStatus::Failed);
}

#[tokio::test]
async fn ensure_running_tracks_restart_count() {
    let mut def = create_test_stdio_def();
    def.auto_restart = true;
    def.max_restart_attempts = 2; // Allow 2 retries
    let max_restarts = def.max_restart_attempts;
    let mut lifecycle = ServerLifecycle::new(def);
    let result = lifecycle.ensure_running().await;
    assert!(result.is_err());
    // After all retries exhausted, restart_count should be >= max_restart_attempts
    assert!(
        lifecycle.restart_count() >= max_restarts,
        "restart_count ({}) should be >= max_restart_attempts ({})",
        lifecycle.restart_count(),
        max_restarts
    );
    assert_eq!(*lifecycle.status(), McpServerStatus::Failed);
}

#[tokio::test]
async fn ensure_running_returns_error_after_max_retries_exceeded() {
    let mut def = create_test_stdio_def();
    def.auto_restart = false;
    def.max_restart_attempts = 3;
    let mut lifecycle = ServerLifecycle::new(def);
    // First ensure_running fails and sets status to Failed
    let result = lifecycle.ensure_running().await;
    assert!(result.is_err());
    assert_eq!(*lifecycle.status(), McpServerStatus::Failed);

    // Second call should immediately fail with MaxRestartsExceeded
    // since auto_restart is false and restart_count >= max_restart_attempts
    let result = lifecycle.ensure_running().await;
    assert!(result.is_err());
    let err = result.err().unwrap();
    match err {
        McpError::MaxRestartsExceeded(name) => assert_eq!(name, "test-server"),
        other => panic!("expected MaxRestartsExceeded, got: {other}"),
    }
}

#[test]
fn def_returns_server_definition() {
    let def = create_test_stdio_def();
    let lifecycle = ServerLifecycle::new(def);
    assert_eq!(lifecycle.def().name, "test-server");
}

#[test]
fn reset_restart_count() {
    let mut lifecycle = create_test_lifecycle();
    lifecycle.restart_count = 5;
    assert_eq!(lifecycle.restart_count(), 5);
    lifecycle.reset_restart_count();
    assert_eq!(lifecycle.restart_count(), 0);
}

#[tokio::test]
async fn discovery_is_none_when_stopped() {
    let lifecycle = create_test_lifecycle();
    assert!(lifecycle.discovery().is_none());
}

#[test]
fn sse_def_without_sse_feature() {
    let def = McpServerDef {
        name: "sse-server".into(),
        transport: McpTransportDef::Sse {
            url: "http://localhost:8080/sse".into(),
            api_key_env: Some("MY_API_KEY".into()),
            headers: HashMap::new(),
        },
        args: vec![],
        env: HashMap::new(),
        keep_alive: true,
        idle_timeout_secs: 300,
        auto_restart: false,
        max_restart_attempts: 3,
    };
    let lifecycle = ServerLifecycle::new(def);
    assert_eq!(*lifecycle.status(), McpServerStatus::Stopped);
    assert_eq!(lifecycle.server_id(), "sse-server");
}

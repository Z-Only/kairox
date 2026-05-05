//! MCP server lifecycle management.
//!
//! Handles starting, stopping, health-checking, and automatic restart of
//! MCP server processes. [`ServerLifecycle`] manages the full lifecycle:
//! on-demand start, idle timeout, crash restart, and graceful shutdown.

use crate::client::McpClient;
use crate::discovery::DiscoveryCache;
use crate::transport::Transport;
use crate::types::*;
use crate::{McpError, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "sse")]
use crate::transport::sse::SseTransport;
use crate::transport::stdio::StdioTransport;

/// Manages the lifecycle of an MCP server connection (start, stop, restart, idle timeout).
///
/// Each `ServerLifecycle` is responsible for a single MCP server defined by an
/// [`McpServerDef`]. It handles on-demand startup, idle-timeout shutdown,
/// automatic restart after failure, and graceful teardown.
pub struct ServerLifecycle {
    /// The server definition (configuration).
    def: McpServerDef,
    /// The MCP client, once the server has been started and handshaken.
    client: Option<Arc<McpClient>>,
    /// Discovery cache for this server's tools/resources/prompts.
    discovery: Option<DiscoveryCache>,
    /// Current lifecycle status.
    status: McpServerStatus,
    /// Timestamp of the last activity (request) on this server.
    last_activity: Option<Instant>,
    /// How many restart attempts have been made since the last successful start.
    restart_count: u32,
}

impl ServerLifecycle {
    /// Create a new lifecycle manager for the given server definition.
    ///
    /// The server starts in [`McpServerStatus::Stopped`]; call [`ensure_running`]
    /// to start it on first use.
    ///
    /// [`ensure_running`]: ServerLifecycle::ensure_running
    pub fn new(def: McpServerDef) -> Self {
        Self {
            def,
            client: None,
            discovery: None,
            status: McpServerStatus::Stopped,
            last_activity: None,
            restart_count: 0,
        }
    }

    /// Ensure the server is running. Starts it if stopped or failed.
    ///
    /// On the first call (or after a stop/failure), this:
    /// 1. Creates a transport based on [`McpTransportDef`]
    /// 2. Creates an [`McpClient`] and performs the MCP handshake
    /// 3. On success: sets status to `Running`, creates a [`DiscoveryCache`],
    ///    resets `restart_count`
    /// 4. On failure: if `auto_restart` and `restart_count < max_restart_attempts`,
    ///    retries; otherwise sets status to `Failed`
    pub async fn ensure_running(&mut self) -> Result<Arc<McpClient>> {
        // If already running, return the existing client.
        if self.status == McpServerStatus::Running {
            if let Some(ref client) = self.client {
                return Ok(Arc::clone(client));
            }
            // Invariant violation — status is Running but no client.
            // Fall through to restart.
        }

        // If we've exhausted restart attempts, refuse to start.
        if self.status == McpServerStatus::Failed
            && !self.def.auto_restart
            && self.restart_count >= self.def.max_restart_attempts
        {
            return Err(McpError::MaxRestartsExceeded(self.def.name.clone()));
        }

        self.status = McpServerStatus::Starting;

        loop {
            match self.try_start().await {
                Ok(client) => {
                    self.status = McpServerStatus::Running;
                    self.restart_count = 0;
                    let client = Arc::new(client);
                    self.discovery = Some(DiscoveryCache::new(Arc::clone(&client)));
                    self.client = Some(Arc::clone(&client));
                    self.mark_active();
                    return Ok(client);
                }
                Err(e) => {
                    self.restart_count += 1;
                    if self.def.auto_restart && self.restart_count < self.def.max_restart_attempts {
                        tracing::warn!(
                            target: "mcp::lifecycle",
                            "Server '{}' start failed (attempt {}/{}): {}. Retrying...",
                            self.def.name,
                            self.restart_count,
                            self.def.max_restart_attempts,
                            e
                        );
                        continue;
                    }
                    self.status = McpServerStatus::Failed;
                    self.client = None;
                    self.discovery = None;
                    return Err(McpError::MaxRestartsExceeded(self.def.name.clone()));
                }
            }
        }
    }

    /// Attempt to start the server by creating a transport and performing handshake.
    async fn try_start(&self) -> Result<McpClient> {
        let transport = self.create_transport().await?;
        let client = McpClient::new(&self.def.name, transport);
        client.handshake().await?;
        Ok(client)
    }

    /// Create the appropriate transport based on the server definition.
    async fn create_transport(&self) -> Result<Box<dyn Transport>> {
        match &self.def.transport {
            McpTransportDef::Stdio { command, cwd } => {
                let args: Vec<&str> = self.def.args.iter().map(|s| s.as_str()).collect();
                let cwd_str = cwd.as_deref();
                let transport =
                    StdioTransport::spawn(command, &args, self.def.env.clone(), cwd_str).await?;
                Ok(Box::new(transport))
            }
            McpTransportDef::Sse {
                url,
                api_key_env,
                headers,
            } => {
                #[cfg(feature = "sse")]
                {
                    let transport =
                        SseTransport::new(url, headers.clone(), api_key_env.as_deref()).await?;
                    Ok(Box::new(transport))
                }
                #[cfg(not(feature = "sse"))]
                {
                    let _ = (url, api_key_env, headers);
                    Err(McpError::Transport(
                        "SSE transport requires the 'sse' feature".into(),
                    ))
                }
            }
        }
    }

    /// Record that the server was just used (updates the idle timer).
    pub fn mark_active(&mut self) {
        self.last_activity = Some(Instant::now());
    }

    /// Check idle timeout. If the server has been idle longer than
    /// `idle_timeout_secs` and `keep_alive` is false, shut it down.
    pub async fn check_idle_timeout(&mut self) -> Result<()> {
        if self.def.keep_alive {
            return Ok(());
        }

        if let Some(last) = self.last_activity {
            let elapsed = last.elapsed();
            if elapsed > Duration::from_secs(self.def.idle_timeout_secs) {
                tracing::info!(
                    target: "mcp::lifecycle",
                    "Server '{}' idle for {:?}s, shutting down",
                    self.def.name,
                    elapsed.as_secs()
                );
                self.shutdown().await?;
            }
        }

        Ok(())
    }

    /// Gracefully shut down the server.
    ///
    /// Sends a shutdown notification and closes the transport. Sets status
    /// to [`McpServerStatus::Stopped`].
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(client) = self.client.take() {
            // Best-effort shutdown — ignore errors since the server may
            // already be gone.
            if let Err(e) = client.shutdown().await {
                tracing::debug!(
                    target: "mcp::lifecycle",
                    "Shutdown notification for '{}' failed (server may already be gone): {e}",
                    self.def.name
                );
            }
        }
        self.discovery = None;
        self.status = McpServerStatus::Stopped;
        // Don't reset restart_count — we want to track cumulative failures
        // across the lifetime of this ServerLifecycle instance.
        Ok(())
    }

    /// Get the current server status.
    pub fn status(&self) -> &McpServerStatus {
        &self.status
    }

    /// Get the server ID (the friendly name from the definition).
    pub fn server_id(&self) -> &str {
        &self.def.name
    }

    /// Get the discovery cache (if the server is running).
    pub fn discovery(&self) -> Option<&DiscoveryCache> {
        self.discovery.as_ref()
    }

    /// Get a reference to the server definition.
    pub fn def(&self) -> &McpServerDef {
        &self.def
    }

    /// Get the number of restart attempts so far.
    pub fn restart_count(&self) -> u32 {
        self.restart_count
    }

    /// Get the time of last activity, if any.
    pub fn last_activity(&self) -> Option<Instant> {
        self.last_activity
    }

    /// Reset the restart counter. Call this after a successful operation
    /// to indicate the server is healthy.
    pub fn reset_restart_count(&mut self) {
        self.restart_count = 0;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
}

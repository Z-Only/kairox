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
#[cfg(feature = "sse")]
use crate::transport::streamable_http::StreamableHttpTransport;

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
            McpTransportDef::StreamableHttp {
                url,
                api_key_env,
                headers,
            } => {
                #[cfg(feature = "sse")]
                {
                    let transport =
                        StreamableHttpTransport::new(url, headers.clone(), api_key_env.as_deref())
                            .await?;
                    Ok(Box::new(transport))
                }
                #[cfg(not(feature = "sse"))]
                {
                    let _ = (url, api_key_env, headers);
                    Err(McpError::Transport(
                        "Streamable HTTP transport requires the 'sse' feature".into(),
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

    /// Get the MCP client (if the server is running).
    pub fn client(&self) -> Option<Arc<McpClient>> {
        self.client.clone()
    }

    /// Get the discovery cache (if the server is running).
    pub fn discovery(&self) -> Option<&DiscoveryCache> {
        self.discovery.as_ref()
    }

    /// Get cached tools, starting the server if needed.
    pub async fn cached_tools(&mut self) -> Result<Vec<McpToolDef>> {
        self.ensure_running().await?;
        let discovery = self.discovery.as_ref().ok_or_else(|| {
            McpError::Protocol(format!(
                "server '{}' is running without discovery cache",
                self.def.name
            ))
        })?;
        let tools = discovery.tools().await?;
        self.mark_active();
        Ok(tools)
    }

    /// Force-refresh the cached tool list, starting the server if needed.
    pub async fn refresh_cached_tools(&mut self) -> Result<Vec<McpToolDef>> {
        self.ensure_running().await?;
        let discovery = self.discovery.as_ref().ok_or_else(|| {
            McpError::Protocol(format!(
                "server '{}' is running without discovery cache",
                self.def.name
            ))
        })?;
        discovery.invalidate_tools().await;
        let tools = discovery.tools().await?;
        self.mark_active();
        Ok(tools)
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

    /// Test connectivity to the MCP server.
    ///
    /// Delegates to [`crate::health::check_connectivity`]. See that function
    /// for full documentation.
    pub async fn test_connectivity(&mut self, timeout: Option<Duration>) -> ConnectivityResult {
        crate::health::check_connectivity(self, timeout).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;

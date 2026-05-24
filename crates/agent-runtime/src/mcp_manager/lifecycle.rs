//! Server lifecycle: start, stop, health, idle-timeout, and connectivity checks.

use super::McpServerManager;
use agent_core::EventPayload;
use agent_mcp::types::{CheckHealthResult, McpServerStatus};
use agent_mcp::{McpClient, McpError};
use agent_tools::provider::mcp_provider::McpToolAdapter;
use std::sync::Arc;

impl McpServerManager {
    /// Start all servers marked as `keep_alive`.
    ///
    /// Returns a vec of results (one per keep-alive server). Errors are
    /// logged but do not prevent other servers from starting.
    pub async fn start_persistent_servers(&mut self) -> Vec<Result<(), McpError>> {
        let keep_alive_ids: Vec<String> = self
            .servers
            .iter()
            .filter(|(_, lc)| lc.def().keep_alive)
            .map(|(id, _)| id.clone())
            .collect();

        let mut results = Vec::new();
        for id in keep_alive_ids {
            let result = self.ensure_server(&id).await.map(|_| ()).map_err(|e| {
                tracing::error!("Failed to start keep_alive MCP server '{}': {}", id, e);
                e
            });
            results.push(result);
        }
        results
    }

    /// Ensure a server is running (start if stopped or failed).
    ///
    /// On first successful start, discovers tools from the server and
    /// registers them in the tool registry.
    pub async fn ensure_server(&mut self, server_id: &str) -> Result<Arc<McpClient>, McpError> {
        // Check if the server exists first
        if !self.servers.contains_key(server_id) {
            return Err(McpError::NotRunning(server_id.to_string()));
        }

        // Get current status before starting
        let was_stopped = {
            let lifecycle = self.servers.get(server_id).unwrap();
            *lifecycle.status() == McpServerStatus::Stopped
                || matches!(lifecycle.status(), McpServerStatus::Failed)
        };

        self.emit_event(EventPayload::McpServerStarting {
            server_id: server_id.to_string(),
        });

        // Start the server
        let client = {
            let lifecycle = self.servers.get_mut(server_id).unwrap();
            lifecycle.ensure_running().await.inspect_err(|e| {
                self.emit_event(EventPayload::McpServerFailed {
                    server_id: server_id.to_string(),
                    error: e.to_string(),
                });
            })?
        };

        if was_stopped {
            // Register tools from this server (skip disabled tools)
            let tools_result = {
                let lifecycle = self.servers.get_mut(server_id).unwrap();
                lifecycle.cached_tools().await
            };
            match tools_result {
                Ok(tools) => {
                    let disabled = self
                        .disabled_tools
                        .get(server_id)
                        .cloned()
                        .unwrap_or_default();
                    let enabled_tools: Vec<_> = tools
                        .into_iter()
                        .filter(|t| !disabled.contains(&t.name))
                        .collect();
                    let tool_count = enabled_tools.len();
                    let mut registry = self.tool_registry.lock().await;
                    for tool_def in enabled_tools {
                        let adapter =
                            McpToolAdapter::new(server_id.to_string(), tool_def, client.clone());
                        registry.register(Box::new(adapter));
                    }
                    self.emit_event(EventPayload::McpServerReady {
                        server_id: server_id.to_string(),
                        tool_count,
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        "MCP server '{}' is running but tool discovery failed: {}",
                        server_id,
                        e
                    );
                    // Still emit ready with 0 tools — the server is up, just no tools
                    self.emit_event(EventPayload::McpServerReady {
                        server_id: server_id.to_string(),
                        tool_count: 0,
                    });
                }
            }
        }

        Ok(client)
    }

    /// Check idle timeouts for all servers.
    ///
    /// Servers that have been idle longer than their `idle_timeout_secs`
    /// (and are not `keep_alive`) will be shut down automatically.
    pub async fn check_idle_timeouts(&mut self) -> Result<(), McpError> {
        let server_ids: Vec<String> = self.servers.keys().cloned().collect();
        for id in server_ids {
            if let Some(lifecycle) = self.servers.get_mut(&id) {
                let was_running = *lifecycle.status() == McpServerStatus::Running;
                lifecycle.check_idle_timeout().await?;
                if was_running && *lifecycle.status() == McpServerStatus::Stopped {
                    self.emit_event(EventPayload::McpServerStopped {
                        server_id: id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Stop a specific server.
    pub async fn shutdown_server(&mut self, server_id: &str) -> Result<(), McpError> {
        let lifecycle = self
            .servers
            .get_mut(server_id)
            .ok_or_else(|| McpError::NotRunning(server_id.to_string()))?;
        lifecycle.shutdown().await?;
        self.emit_event(EventPayload::McpServerStopped {
            server_id: server_id.to_string(),
        });
        Ok(())
    }

    /// Shut down all managed servers.
    pub async fn shutdown_all(&mut self) -> Result<(), McpError> {
        let ids: Vec<String> = self.servers.keys().cloned().collect();
        for id in ids {
            if let Some(lifecycle) = self.servers.get_mut(&id) {
                let _ = lifecycle.shutdown().await;
                self.emit_event(EventPayload::McpServerStopped { server_id: id });
            }
        }
        Ok(())
    }

    /// Test connectivity to an MCP server.
    ///
    /// Starts the server if necessary, then calls `tools/list` to verify
    /// the server is responsive. Returns a [`agent_mcp::ConnectivityResult`].
    pub async fn test_connectivity(
        &mut self,
        server_id: &str,
        timeout: Option<std::time::Duration>,
    ) -> Result<agent_mcp::ConnectivityResult, McpError> {
        let lifecycle = self
            .servers
            .get_mut(server_id)
            .ok_or_else(|| McpError::NotRunning(server_id.to_string()))?;
        Ok(lifecycle.test_connectivity(timeout).await)
    }

    /// Check server health: try to start + discover tools.
    ///
    /// Returns [`CheckHealthResult`] with the full tool list on success,
    /// or `healthy: false` with an error message. Does NOT register tools
    /// in the tool registry — that's done by [`Self::ensure_server`].
    pub async fn check_health(
        &mut self,
        server_id: &str,
        timeout: Option<std::time::Duration>,
    ) -> CheckHealthResult {
        let lifecycle = match self.servers.get_mut(server_id) {
            Some(lc) => lc,
            None => {
                return CheckHealthResult {
                    tools: Vec::new(),
                    healthy: false,
                    error: Some(format!("server '{server_id}' not found")),
                }
            }
        };

        let default_timeout = std::time::Duration::from_secs(15);
        let timeout = timeout.unwrap_or(default_timeout);

        // Ensure the server is running
        match tokio::time::timeout(timeout, lifecycle.ensure_running()).await {
            Ok(Ok(_client)) => {}
            Ok(Err(e)) => {
                return CheckHealthResult {
                    tools: Vec::new(),
                    healthy: false,
                    error: Some(format!("failed to start server: {e}")),
                }
            }
            Err(_elapsed) => {
                return CheckHealthResult {
                    tools: Vec::new(),
                    healthy: false,
                    error: Some(format!(
                        "timed out after {}s waiting for server to start",
                        timeout.as_secs()
                    )),
                }
            }
        };

        // Discover tools, using the lifecycle discovery cache when warm.
        match tokio::time::timeout(timeout, lifecycle.cached_tools()).await {
            Ok(Ok(tools)) => {
                lifecycle.mark_active();
                CheckHealthResult {
                    tools,
                    healthy: true,
                    error: None,
                }
            }
            Ok(Err(e)) => CheckHealthResult {
                tools: Vec::new(),
                healthy: false,
                error: Some(format!("tool discovery failed: {e}")),
            },
            Err(_elapsed) => CheckHealthResult {
                tools: Vec::new(),
                healthy: false,
                error: Some(format!(
                    "timed out after {}s waiting for tool discovery",
                    timeout.as_secs()
                )),
            },
        }
    }
}

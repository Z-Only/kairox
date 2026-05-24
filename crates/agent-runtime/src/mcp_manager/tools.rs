//! Tool refresh and per-server enable/disable management.

use super::McpServerManager;
use agent_mcp::types::{McpServerStatus, McpToolDef};
use agent_mcp::McpError;
use agent_tools::provider::mcp_provider::McpToolAdapter;
use std::collections::HashSet;

impl McpServerManager {
    /// Refresh the tool list from a running server.
    ///
    /// Re-registers tools (the `ToolRegistry` handles dedup by tool_id).
    pub async fn refresh_tools(&mut self, server_id: &str) -> Result<Vec<McpToolDef>, McpError> {
        let (client, tools) = {
            let lifecycle = self
                .servers
                .get_mut(server_id)
                .ok_or_else(|| McpError::NotRunning(server_id.to_string()))?;
            let tools = lifecycle.refresh_cached_tools().await.map_err(|e| {
                tracing::error!(
                    "Failed to refresh tools from MCP server '{}': {}",
                    server_id,
                    e
                );
                e
            })?;
            let client = lifecycle
                .client()
                .ok_or_else(|| McpError::NotRunning(server_id.to_string()))?;
            (client, tools)
        };

        let disabled = self
            .disabled_tools
            .get(server_id)
            .cloned()
            .unwrap_or_default();
        let mut registry = self.tool_registry.lock().await;
        for tool_def in &tools {
            if !disabled.contains(&tool_def.name) {
                let adapter =
                    McpToolAdapter::new(server_id.to_string(), tool_def.clone(), client.clone());
                registry.register(Box::new(adapter));
            }
        }

        Ok(tools)
    }

    /// Load disabled tools for a server from external config.
    pub fn load_disabled_tools(&mut self, server_id: &str, disabled: HashSet<String>) {
        self.disabled_tools.insert(server_id.to_string(), disabled);
    }

    /// Get disabled tool names for a server.
    pub fn get_disabled_tools(&self, server_id: &str) -> HashSet<String> {
        self.disabled_tools
            .get(server_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Enable or disable a specific tool on a server.
    ///
    /// When disabling, the tool is unregistered from the ToolRegistry.
    /// When enabling, re-discovers tools from the running server and
    /// registers the matching one.
    pub async fn set_tool_disabled(
        &mut self,
        server_id: &str,
        tool_name: &str,
        disabled: bool,
    ) -> Result<(), McpError> {
        let tools = self
            .disabled_tools
            .entry(server_id.to_string())
            .or_default();

        if disabled {
            tools.insert(tool_name.to_string());
            let tool_id = format!("mcp.{server_id}.{tool_name}");
            let mut registry = self.tool_registry.lock().await;
            registry.unregister(&tool_id);
        } else {
            tools.remove(tool_name);
            // Re-register if server is running by re-discovering tools
            if let Some(lifecycle) = self.servers.get(server_id) {
                if *lifecycle.status() == McpServerStatus::Running {
                    if let Some(client) = lifecycle.client() {
                        if let Ok(discovered) = client.discover_tools().await {
                            let mut registry = self.tool_registry.lock().await;
                            for tool_def in discovered {
                                if tool_def.name == tool_name {
                                    let adapter = McpToolAdapter::new(
                                        server_id.to_string(),
                                        tool_def,
                                        client.clone(),
                                    );
                                    registry.register(Box::new(adapter));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

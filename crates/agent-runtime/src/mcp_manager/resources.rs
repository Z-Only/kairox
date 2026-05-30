//! Resource and prompt discovery on running MCP servers.

use super::McpServerManager;
use agent_mcp::types::{McpContentBlock, McpPromptDef, McpResourceDef};
use agent_mcp::McpError;

impl McpServerManager {
    /// List resources from a running server.
    pub async fn list_resources(&self, server_id: &str) -> Result<Vec<McpResourceDef>, McpError> {
        let lifecycle = self
            .servers
            .get(server_id)
            .ok_or_else(|| McpError::NotRunning(server_id.to_string()))?;
        let client = lifecycle
            .client()
            .ok_or_else(|| McpError::NotRunning(server_id.to_string()))?;
        client.discover_resources().await
    }

    /// List prompts from a running server.
    pub async fn list_prompts(&self, server_id: &str) -> Result<Vec<McpPromptDef>, McpError> {
        let lifecycle = self
            .servers
            .get(server_id)
            .ok_or_else(|| McpError::NotRunning(server_id.to_string()))?;
        let client = lifecycle
            .client()
            .ok_or_else(|| McpError::NotRunning(server_id.to_string()))?;
        client.discover_prompts().await
    }

    /// Read a resource from a running server.
    pub async fn read_resource(
        &self,
        server_id: &str,
        uri: &str,
    ) -> Result<Vec<McpContentBlock>, McpError> {
        let lifecycle = self
            .servers
            .get(server_id)
            .ok_or_else(|| McpError::NotRunning(server_id.to_string()))?;
        let client = lifecycle
            .client()
            .ok_or_else(|| McpError::NotRunning(server_id.to_string()))?;
        client.read_resource(uri).await
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;

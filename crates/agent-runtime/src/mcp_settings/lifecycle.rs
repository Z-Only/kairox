use agent_core::CoreError;

use crate::McpServerManager;

#[async_trait::async_trait]
pub trait McpSettingsLifecycle {
    fn is_server_running(&self, server_id: &str) -> bool;

    async fn stop_server(&mut self, server_id: &str) -> agent_core::Result<()>;
}

#[async_trait::async_trait]
impl McpSettingsLifecycle for McpServerManager {
    fn is_server_running(&self, server_id: &str) -> bool {
        self.is_running(server_id).unwrap_or(false)
    }

    async fn stop_server(&mut self, server_id: &str) -> agent_core::Result<()> {
        self.shutdown_server(server_id)
            .await
            .map_err(|error| CoreError::InvalidState(format!("failed to stop MCP server: {error}")))
    }
}

pub(super) struct NoopMcpSettingsLifecycle;

#[async_trait::async_trait]
impl McpSettingsLifecycle for NoopMcpSettingsLifecycle {
    fn is_server_running(&self, _server_id: &str) -> bool {
        false
    }

    async fn stop_server(&mut self, _server_id: &str) -> agent_core::Result<()> {
        Ok(())
    }
}

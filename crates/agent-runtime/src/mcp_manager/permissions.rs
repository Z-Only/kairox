//! Trust/permission management for MCP servers.

use super::McpServerManager;
use agent_core::EventPayload;
use agent_mcp::McpError;
use agent_tools::permission::PermissionEngine;
use std::sync::Arc;
use tokio::sync::Mutex;

impl McpServerManager {
    /// Trust a server (auto-approve its MCP tool invocations).
    pub async fn trust_server(&self, server_id: &str) -> Result<(), McpError> {
        let mut engine = self.permission_engine.lock().await;
        engine.trust_server(server_id.to_string());
        drop(engine);
        self.emit_event(EventPayload::McpTrustGranted {
            server_id: server_id.to_string(),
        });
        Ok(())
    }

    /// Revoke trust from a server.
    pub async fn revoke_trust(&self, server_id: &str) -> Result<(), McpError> {
        let mut engine = self.permission_engine.lock().await;
        engine.revoke_trust(server_id);
        drop(engine);
        self.emit_event(EventPayload::McpTrustRevoked {
            server_id: server_id.to_string(),
        });
        Ok(())
    }

    /// Check whether a server is trusted.
    pub async fn is_trusted(&self, server_id: &str) -> bool {
        let engine = self.permission_engine.lock().await;
        engine.trusted_servers().contains(server_id)
    }

    /// Return a clone of the permission engine handle for settings snapshots.
    pub(crate) fn permission_engine(&self) -> Arc<Mutex<PermissionEngine>> {
        Arc::clone(&self.permission_engine)
    }
}

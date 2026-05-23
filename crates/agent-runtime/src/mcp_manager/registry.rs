//! Server registry queries and dynamic registration/unregistration.

use super::McpServerManager;
use agent_mcp::lifecycle::ServerLifecycle;
use agent_mcp::types::{McpServerDef, McpServerStatus};
use agent_mcp::McpError;
use std::collections::HashMap;

impl McpServerManager {
    /// Get the status of all managed servers.
    pub fn server_statuses(&self) -> HashMap<String, McpServerStatus> {
        self.servers
            .iter()
            .map(|(id, lc)| (id.clone(), *lc.status()))
            .collect()
    }

    /// Get the number of managed servers.
    pub fn server_count(&self) -> usize {
        self.servers.len()
    }

    /// Get the server definition for a given server id.
    pub fn server_def(&self, server_id: &str) -> Option<&McpServerDef> {
        self.servers.get(server_id).map(|lc| lc.def())
    }

    /// Returns true if a server with this id is currently registered (in any state).
    pub fn is_registered(&self, server_id: &str) -> bool {
        self.servers.contains_key(server_id)
    }

    /// Returns `Some(true)` if the server is registered and currently running,
    /// `Some(false)` if registered but not running, or `None` if unknown.
    pub fn is_running(&self, server_id: &str) -> Option<bool> {
        self.servers
            .get(server_id)
            .map(|lc| matches!(lc.status(), McpServerStatus::Running))
    }

    /// Register a server definition at runtime (used by the marketplace installer).
    ///
    /// Returns `Err` if a server with the same id is already registered.
    /// The caller is responsible for invoking [`Self::ensure_server`] to start it.
    pub fn register_dynamic(&mut self, def: McpServerDef) -> Result<(), McpError> {
        if self.servers.contains_key(&def.name) {
            return Err(McpError::Protocol(format!(
                "server '{}' is already registered",
                def.name
            )));
        }
        let name = def.name.clone();
        self.servers.insert(name, ServerLifecycle::new(def));
        Ok(())
    }

    /// Remove a dynamically registered server. Stops it first if running.
    pub async fn unregister_dynamic(&mut self, server_id: &str) -> Result<(), McpError> {
        if let Some(lifecycle) = self.servers.get_mut(server_id) {
            if matches!(lifecycle.status(), McpServerStatus::Running) {
                let _ = lifecycle.shutdown().await;
            }
        }
        self.servers.remove(server_id);
        Ok(())
    }
}

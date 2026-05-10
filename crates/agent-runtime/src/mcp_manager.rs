//! MCP Server Manager — orchestrates MCP server lifecycle, tool registration, and permission management.
//!
//! [`McpServerManager`] is the central integration point that ties together
//! `agent-mcp` (lifecycle), `agent-tools` (registry + permissions), and
//! `agent-config` (server definitions). It owns a collection of
//! [`ServerLifecycle`] instances and provides high-level operations:
//! start/stop servers, register discovered tools, manage trust, and emit
//! domain events.

use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
};
use agent_mcp::lifecycle::ServerLifecycle;
use agent_mcp::types::{
    McpContentBlock, McpPromptDef, McpResourceDef, McpServerDef, McpServerStatus, McpToolDef,
};
use agent_mcp::{McpClient, McpError};
use agent_tools::permission::PermissionEngine;
use agent_tools::provider::mcp_provider::McpToolAdapter;
use agent_tools::registry::ToolRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Orchestrates MCP server lifecycle, tool registration, and permission management.
///
/// Created from a list of [`McpServerDef`] (parsed from `kairox.toml`), and
/// holds references to the shared [`ToolRegistry`], [`PermissionEngine`], and
/// event broadcast channel so it can register tools, check/manage permissions,
/// and emit lifecycle events as servers start, stop, or fail.
pub struct McpServerManager {
    servers: HashMap<String, ServerLifecycle>,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    permission_engine: Arc<Mutex<PermissionEngine>>,
    event_tx: Option<tokio::sync::broadcast::Sender<DomainEvent>>,
}

impl McpServerManager {
    /// Create a new manager from parsed config definitions.
    ///
    /// All servers start in `Stopped` state; call [`start_persistent_servers`]
    /// or [`ensure_server`] to start them.
    pub fn from_config(
        configs: Vec<McpServerDef>,
        tool_registry: Arc<Mutex<ToolRegistry>>,
        permission_engine: Arc<Mutex<PermissionEngine>>,
        event_tx: Option<tokio::sync::broadcast::Sender<DomainEvent>>,
    ) -> Self {
        let servers: HashMap<String, ServerLifecycle> = configs
            .into_iter()
            .map(|def| (def.name.clone(), ServerLifecycle::new(def)))
            .collect();
        Self {
            servers,
            tool_registry,
            permission_engine,
            event_tx,
        }
    }

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
            // Register tools from this server
            match client.discover_tools().await {
                Ok(tools) => {
                    let tool_count = tools.len();
                    let mut registry = self.tool_registry.lock().await;
                    for tool_def in tools {
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

    /// Refresh the tool list from a running server.
    ///
    /// Re-registers tools (the `ToolRegistry` handles dedup by tool_id).
    pub async fn refresh_tools(&mut self, server_id: &str) -> Result<Vec<McpToolDef>, McpError> {
        let client = self.ensure_server(server_id).await?;
        let tools = client.discover_tools().await.map_err(|e| {
            tracing::error!(
                "Failed to refresh tools from MCP server '{}': {}",
                server_id,
                e
            );
            e
        })?;

        let mut registry = self.tool_registry.lock().await;
        for tool_def in &tools {
            let adapter =
                McpToolAdapter::new(server_id.to_string(), tool_def.clone(), client.clone());
            registry.register(Box::new(adapter));
        }

        Ok(tools)
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

    /// Get the status of all managed servers.
    pub fn server_statuses(&self) -> HashMap<String, McpServerStatus> {
        self.servers
            .iter()
            .map(|(id, lc)| (id.clone(), *lc.status()))
            .collect()
    }

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

    /// Emit a domain event via the broadcast channel (best-effort).
    fn emit_event(&self, payload: EventPayload) {
        if let Some(tx) = &self.event_tx {
            let event = DomainEvent::new(
                WorkspaceId::new(),
                SessionId::new(),
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                payload,
            );
            let _ = tx.send(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_mcp::types::McpTransportDef;
    use agent_tools::PermissionMode;
    use std::collections::HashMap;

    fn make_test_def(name: &str, keep_alive: bool) -> McpServerDef {
        McpServerDef {
            name: name.to_string(),
            transport: McpTransportDef::Stdio {
                command: "echo".to_string(),
                cwd: None,
            },
            args: vec![],
            env: HashMap::new(),
            keep_alive,
            idle_timeout_secs: 300,
            auto_restart: false,
            max_restart_attempts: 0,
        }
    }

    fn make_manager(configs: Vec<McpServerDef>) -> McpServerManager {
        let (event_tx, _) = tokio::sync::broadcast::channel(64);
        McpServerManager::from_config(
            configs,
            Arc::new(Mutex::new(ToolRegistry::new())),
            Arc::new(Mutex::new(PermissionEngine::new(PermissionMode::Suggest))),
            Some(event_tx),
        )
    }

    #[test]
    fn from_config_creates_all_servers_stopped() {
        let configs = vec![make_test_def("srv-a", false), make_test_def("srv-b", true)];
        let manager = make_manager(configs);
        assert_eq!(manager.server_count(), 2);
        let statuses = manager.server_statuses();
        assert_eq!(*statuses.get("srv-a").unwrap(), McpServerStatus::Stopped);
        assert_eq!(*statuses.get("srv-b").unwrap(), McpServerStatus::Stopped);
    }

    #[test]
    fn server_statuses_returns_all_servers() {
        let configs = vec![make_test_def("alpha", false), make_test_def("beta", true)];
        let manager = make_manager(configs);
        let statuses = manager.server_statuses();
        assert_eq!(statuses.len(), 2);
        assert!(statuses.contains_key("alpha"));
        assert!(statuses.contains_key("beta"));
    }

    #[tokio::test]
    async fn trust_and_revoke_server() {
        let manager = make_manager(vec![make_test_def("trusted-srv", false)]);
        assert!(!manager.is_trusted("trusted-srv").await);

        manager.trust_server("trusted-srv").await.unwrap();
        assert!(manager.is_trusted("trusted-srv").await);

        manager.revoke_trust("trusted-srv").await.unwrap();
        assert!(!manager.is_trusted("trusted-srv").await);
    }

    #[tokio::test]
    async fn trust_unknown_server_is_noop() {
        let manager = make_manager(vec![]);
        // Trusting a server that's not in the manager — permission engine still records it
        manager.trust_server("unknown").await.unwrap();
        assert!(manager.is_trusted("unknown").await);
    }

    #[tokio::test]
    async fn shutdown_all_on_empty_is_ok() {
        let mut manager = make_manager(vec![]);
        manager.shutdown_all().await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_all_stops_all_servers() {
        let configs = vec![make_test_def("srv-1", false), make_test_def("srv-2", false)];
        let mut manager = make_manager(configs);
        manager.shutdown_all().await.unwrap();
        let statuses = manager.server_statuses();
        for status in statuses.values() {
            assert_eq!(*status, McpServerStatus::Stopped);
        }
    }

    #[tokio::test]
    async fn ensure_server_unknown_returns_error() {
        let mut manager = make_manager(vec![]);
        let result = manager.ensure_server("nonexistent").await;
        let err = result.err().unwrap();
        match err {
            McpError::NotRunning(name) => assert_eq!(name, "nonexistent"),
            other => panic!("expected NotRunning, got: {}", other),
        }
    }

    #[test]
    fn server_def_returns_definition() {
        let configs = vec![make_test_def("my-server", true)];
        let manager = make_manager(configs);
        let def = manager.server_def("my-server").unwrap();
        assert_eq!(def.name, "my-server");
        assert!(def.keep_alive);
    }

    #[test]
    fn server_def_unknown_returns_none() {
        let manager = make_manager(vec![]);
        assert!(manager.server_def("unknown").is_none());
    }

    #[tokio::test]
    async fn register_dynamic_adds_server() {
        let mut m = make_manager(vec![]);
        assert!(!m.is_registered("alpha"));
        m.register_dynamic(make_test_def("alpha", false))
            .expect("register");
        assert!(m.is_registered("alpha"));
        assert_eq!(m.server_count(), 1);
    }

    #[tokio::test]
    async fn register_dynamic_rejects_duplicate() {
        let mut m = make_manager(vec![]);
        m.register_dynamic(make_test_def("alpha", false)).unwrap();
        let err = m
            .register_dynamic(make_test_def("alpha", false))
            .unwrap_err();
        assert!(matches!(err, McpError::Protocol(msg) if msg.contains("already registered")));
    }

    #[tokio::test]
    async fn unregister_dynamic_removes_server() {
        let mut m = make_manager(vec![make_test_def("alpha", false)]);
        assert!(m.is_registered("alpha"));
        m.unregister_dynamic("alpha").await.unwrap();
        assert!(!m.is_registered("alpha"));
    }

    #[tokio::test]
    async fn unregister_dynamic_unknown_is_noop() {
        let mut m = make_manager(vec![]);
        m.unregister_dynamic("does-not-exist").await.unwrap();
    }
}

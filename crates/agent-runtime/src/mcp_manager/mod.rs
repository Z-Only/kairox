//! MCP Server Manager — orchestrates MCP server lifecycle, tool registration, and permission management.
//!
//! [`McpServerManager`] is the central integration point that ties together
//! `agent-mcp` (lifecycle), `agent-tools` (registry + permissions), and
//! `agent-config` (server definitions). It owns a collection of
//! [`ServerLifecycle`] instances and provides high-level operations:
//! start/stop servers, register discovered tools, manage trust, and emit
//! domain events.

use agent_core::DomainEvent;
use agent_mcp::lifecycle::ServerLifecycle;
use agent_mcp::types::McpServerDef;
use agent_tools::permission::PermissionEngine;
use agent_tools::registry::ToolRegistry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

mod events;
mod lifecycle;
mod permissions;
mod registry;
mod resources;
mod tools;

#[cfg(test)]
mod tests;

/// Orchestrates MCP server lifecycle, tool registration, and permission management.
///
/// Created from a list of [`McpServerDef`] (parsed from `kairox.toml`), and
/// holds references to the shared [`ToolRegistry`], [`PermissionEngine`], and
/// event broadcast channel so it can register tools, check/manage permissions,
/// and emit lifecycle events as servers start, stop, or fail.
pub struct McpServerManager {
    pub(super) servers: HashMap<String, ServerLifecycle>,
    pub(super) tool_registry: Arc<Mutex<ToolRegistry>>,
    pub(super) permission_engine: Arc<Mutex<PermissionEngine>>,
    pub(super) event_tx: Option<tokio::sync::broadcast::Sender<DomainEvent>>,
    /// Per-server set of disabled tool names. Tools in this set are not
    /// registered in the tool registry when the server starts.
    pub(super) disabled_tools: HashMap<String, HashSet<String>>,
}

impl McpServerManager {
    /// Create a new manager from parsed config definitions.
    ///
    /// All servers start in `Stopped` state; call [`Self::start_persistent_servers`]
    /// or [`Self::ensure_server`] to start them.
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
            disabled_tools: HashMap::new(),
        }
    }
}

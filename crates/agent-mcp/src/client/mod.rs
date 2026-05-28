//! MCP client — high-level interface for interacting with an MCP server.
//!
//! Wraps a [`Transport`][crate::transport::Transport] and provides typed methods
//! for initialization, tool listing/invocation, resource access, and prompt rendering.
//!
//! The client implementation is split across several themed submodules:
//!
//! - [`mod@init`] — protocol handshake.
//! - [`mod@discovery`] — `tools/list`, `resources/list`, `prompts/list`.
//! - [`mod@invocation`] — `tools/call`, `resources/read`, `prompts/get`.
//!
//! All submodules extend the same [`McpClient`] type.

mod discovery;
mod init;
mod invocation;

#[cfg(test)]
mod tests;

use crate::protocol::*;
use crate::transport::Transport;
use crate::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

/// High-level MCP client for a single server connection.
///
/// Each `McpClient` wraps a [`Transport`] and provides typed methods that
/// correspond to the MCP protocol operations: handshake, tool discovery/invocation,
/// resource reading, and prompt rendering.
///
/// The client uses an internal request ID counter that increments atomically,
/// and a `OnceCell` for caching `ServerInfo` after the initial handshake.
/// Discovery methods (tools / resources / prompts) always query the server;
/// use [`DiscoveryCache`][crate::DiscoveryCache] for a caching layer on top.
pub struct McpClient {
    /// A friendly identifier for the server this client is connected to.
    server_id: String,
    /// The underlying transport, wrapped in `Arc<Mutex<...>>` because
    /// [`Transport::send_request`] takes `&mut self`.
    transport: Arc<Mutex<Box<dyn Transport>>>,
    /// Cached server info after a successful handshake.
    server_info: OnceCell<ServerInfo>,
    /// Monotonic request ID counter.
    next_id: AtomicU64,
}

impl McpClient {
    /// Create a new client for the given server, using the provided transport.
    pub fn new(server_id: impl Into<String>, transport: Box<dyn Transport>) -> Self {
        Self {
            server_id: server_id.into(),
            transport: Arc::new(Mutex::new(transport)),
            server_info: OnceCell::new(),
            next_id: AtomicU64::new(1),
        }
    }

    /// Return the server identifier.
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Return the cached `ServerInfo`, if a handshake has been completed.
    pub fn server_info(&self) -> Option<&ServerInfo> {
        self.server_info.get()
    }

    // -- Lifecycle -----------------------------------------------------------

    /// Send a shutdown notification and close the transport.
    pub async fn shutdown(&self) -> Result<()> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/cancelled".to_string(),
            params: Some(serde_json::json!({ "reason": "shutdown" })),
        };
        self.send_notification(notification).await?;
        let mut transport = self.transport.lock().await;
        transport.close().await?;
        Ok(())
    }

    // -- Internal helpers ----------------------------------------------------

    /// Allocate the next request ID.
    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a JSON-RPC request through the transport.
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let mut transport = self.transport.lock().await;
        transport.send_request(request).await
    }

    /// Send a JSON-RPC notification through the transport.
    async fn send_notification(&self, notification: JsonRpcNotification) -> Result<()> {
        let mut transport = self.transport.lock().await;
        transport.send_notification(notification).await
    }
}

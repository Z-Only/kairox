//! MCP transport layer.
//!
//! Defines the [`Transport`] trait for sending JSON-RPC messages to MCP servers,
//! plus concrete implementations for stdio and SSE transports.

pub mod sse;
pub mod stdio;
#[cfg(feature = "sse")]
pub mod streamable_http;

use crate::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, Result};
use async_trait::async_trait;

/// Transport abstraction for communicating with an MCP server.
///
/// Implementations handle the underlying wire protocol (stdio pipe, SSE + HTTP,
/// etc.) and translate JSON-RPC messages to/from the server.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a JSON-RPC request and wait for the response.
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse>;

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()>;

    /// Close the transport connection gracefully.
    async fn close(&mut self) -> Result<()>;
}

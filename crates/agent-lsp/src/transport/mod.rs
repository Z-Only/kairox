pub mod stdio;

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

/// Transport trait for LSP/DAP communication.
///
/// Both LSP and DAP use Content-Length framed messages over stdio,
/// unlike MCP which uses newline-delimited JSON-RPC.
#[async_trait]
pub trait Transport: Send {
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse>;
    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()>;
    async fn close(&mut self) -> Result<()>;
}

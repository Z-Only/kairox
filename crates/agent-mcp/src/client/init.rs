//! Protocol handshake — `initialize` request and `notifications/initialized`.
//!
//! Tries each protocol version from
//! [`MCP_PROTOCOL_VERSION_CANDIDATES`][crate::types::MCP_PROTOCOL_VERSION_CANDIDATES]
//! in turn, falling back to the next when the server reports an unsupported
//! protocol version.

use super::McpClient;
use crate::types::*;
use crate::{McpError, Result};

impl McpClient {
    /// Perform the MCP initialization handshake.
    ///
    /// Sends an `initialize` request, waits for the server's `ServerInfo`,
    /// then sends a `notifications/initialized` notification.
    ///
    /// The `ServerInfo` is cached; subsequent calls return the cached value.
    pub async fn handshake(&self) -> Result<&ServerInfo> {
        self.server_info
            .get_or_try_init(|| async {
                for (index, protocol_version) in MCP_PROTOCOL_VERSION_CANDIDATES.iter().enumerate()
                {
                    let response = match self.send_initialize(protocol_version).await {
                        Ok(response) => response,
                        Err(err)
                            if is_unsupported_protocol_version_error(&err)
                                && index + 1 < MCP_PROTOCOL_VERSION_CANDIDATES.len() =>
                        {
                            continue;
                        }
                        Err(err) => return Err(err),
                    };

                    let result = &response.result;
                    let server_info: ServerInfo = result
                        .get("serverInfo")
                        .ok_or_else(|| {
                            McpError::Handshake(
                                "initialize response missing serverInfo".to_string(),
                            )
                        })
                        .and_then(|v| {
                            serde_json::from_value(v.clone()).map_err(|e| {
                                McpError::Handshake(format!("invalid serverInfo: {e}"))
                            })
                        })?;

                    let notification = JsonRpcNotification {
                        jsonrpc: "2.0".to_string(),
                        method: "notifications/initialized".to_string(),
                        params: None,
                    };
                    self.send_notification(notification).await?;

                    return Ok(server_info);
                }

                Err(McpError::Handshake(
                    "initialize failed for all supported protocol versions".into(),
                ))
            })
            .await
    }

    async fn send_initialize(&self, protocol_version: &str) -> Result<JsonRpcResponse> {
        let id = self.next_id();
        let params = serde_json::json!({
            "protocolVersion": protocol_version,
            "capabilities": {},
            "clientInfo": {
                "name": "kairox",
                "version": env!("CARGO_PKG_VERSION"),
            }
        });
        let request = JsonRpcRequest::new(id, "initialize", Some(params));
        self.send_request(request).await
    }
}

fn is_unsupported_protocol_version_error(err: &McpError) -> bool {
    let message = err.to_string().to_ascii_lowercase();
    message.contains("unsupported protocol version")
        || (message.contains("protocol version") && message.contains("unsupported"))
}

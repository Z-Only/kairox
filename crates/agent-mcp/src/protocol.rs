//! JSON-RPC 2.0 protocol types and MCP server handshake/capability types.
//!
//! These are wire-protocol concerns shared across all MCP transports.
//! Domain-level MCP types (server definitions, tool/resource/prompt discovery,
//! invocation results, lifecycle status) live in [`crate::types`].

use serde::{Deserialize, Serialize};

/// MCP protocol version sent during initialize and HTTP transport negotiation.
pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
/// Protocol versions to try during initialize, newest first.
pub const MCP_PROTOCOL_VERSION_CANDIDATES: &[&str] =
    &[MCP_PROTOCOL_VERSION, "2025-06-18", "2024-11-05"];

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types (internal wire protocol -- no specta derive)
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request with a numeric id.
    pub fn new(id: u64, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Number(id.into()),
            method: method.into(),
            params,
        }
    }

    /// Create a new JSON-RPC request with a string id.
    pub fn new_string_id(
        id: impl Into<String>,
        method: impl Into<String>,
        params: Option<serde_json::Value>,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::String(id.into()),
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 successful response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub result: serde_json::Value,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 notification (no id, no response expected).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// MCP server handshake types
// ---------------------------------------------------------------------------

/// Information about an MCP server, returned during the initialize handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Capabilities advertised by an MCP server during the initialize handshake.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ServerCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
}

/// Server capability for tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ToolsCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Server capability for resources.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ResourcesCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Server capability for prompts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PromptsCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_rpc_request_construction() {
        let req = JsonRpcRequest::new(
            1,
            "initialize",
            Some(json!({"protocolVersion": MCP_PROTOCOL_VERSION})),
        );
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, json!(1));
        assert_eq!(req.method, "initialize");

        let req_str = JsonRpcRequest::new_string_id("abc", "tools/list", None);
        assert_eq!(req_str.id, json!("abc"));
    }

    #[test]
    fn test_server_capabilities_default() {
        let caps = ServerCapabilities::default();
        assert!(caps.tools.is_none());
        assert!(caps.resources.is_none());
        assert!(caps.prompts.is_none());
    }

    #[test]
    fn test_server_capabilities_with_tools() {
        let json = json!({
            "tools": { "list_changed": true }
        });
        let caps: ServerCapabilities = serde_json::from_value(json).unwrap();
        assert!(caps.tools.is_some());
        assert_eq!(caps.tools.as_ref().unwrap().list_changed, Some(true));
    }
}

//! MCP (Model Context Protocol) protocol types.
//!
//! Defines all types for the MCP client including:
//! - JSON-RPC 2.0 message types (internal wire protocol, no specta)
//! - Server handshake/capability types
//! - Server definition and transport config
//! - Tool, resource, and prompt discovery types
//! - Tool invocation result types
//! - Server lifecycle status

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types (internal wire protocol — no specta derive)
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
// MCP server definition / transport config
// ---------------------------------------------------------------------------

/// Definition of an MCP server from the user's configuration (kairox.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpServerDef {
    /// A friendly name used to reference this server configuration.
    pub name: String,
    /// The transport configuration (stdio or sse).
    pub transport: McpTransportDef,
    /// Optional command-line arguments (for stdio transport).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Optional environment variables to set when launching the server.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub env: std::collections::HashMap<String, String>,
    /// Whether to keep the server alive even when idle.
    #[serde(default)]
    pub keep_alive: bool,
    /// Idle timeout in seconds before the server is automatically stopped.
    /// Only applies when `keep_alive` is false. Defaults to 300 seconds.
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
    /// Whether to automatically restart the server on failure.
    #[serde(default = "default_true")]
    pub auto_restart: bool,
    /// Maximum number of restart attempts before giving up.
    #[serde(default = "default_max_restart_attempts")]
    pub max_restart_attempts: u32,
}

/// Default idle timeout in seconds (5 minutes).
const fn default_idle_timeout_secs() -> u64 {
    300
}

/// Default value for `auto_restart`.
const fn default_true() -> bool {
    true
}

/// Default maximum restart attempts.
const fn default_max_restart_attempts() -> u32 {
    3
}

/// Transport configuration for connecting to an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpTransportDef {
    /// Launch the server as a child process communicating over stdin/stdout.
    Stdio {
        /// The command to execute.
        command: String,
        /// Optional working directory for the child process.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
    /// Connect to an already-running server via Server-Sent Events + HTTP POST.
    Sse {
        /// The URL of the SSE endpoint.
        url: String,
        /// Optional environment variable name containing an API key for authentication.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        api_key_env: Option<String>,
        /// Optional HTTP headers to include with every request.
        #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
        headers: std::collections::HashMap<String, String>,
    },
}

// ---------------------------------------------------------------------------
// MCP discovery types
// ---------------------------------------------------------------------------

/// A tool exposed by an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpToolDef {
    /// The tool name, unique within the server.
    pub name: String,
    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON-Schema describing the tool's input parameters, serialized as a string.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "json_string_option",
        alias = "inputSchema"
    )]
    pub input_schema: Option<String>,
}

/// A resource exposed by an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpResourceDef {
    /// The resource URI (e.g. "file:///path/to/resource").
    pub uri: String,
    /// Human-readable name.
    pub name: String,
    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of the resource content.
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "mimeType")]
    pub mime_type: Option<String>,
}

/// A prompt template exposed by an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpPromptDef {
    /// The prompt name, unique within the server.
    pub name: String,
    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Arguments that the prompt accepts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<McpPromptArgument>,
}

/// An argument accepted by an MCP prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpPromptArgument {
    /// The argument name.
    pub name: String,
    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the argument is required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

// ---------------------------------------------------------------------------
// MCP tool invocation result types
// ---------------------------------------------------------------------------

/// Result of invoking an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpToolResult {
    /// The content blocks returned by the tool.
    pub content: Vec<McpContentBlock>,
    /// Whether the tool invocation resulted in an error.
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "isError")]
    pub is_error: Option<bool>,
}

/// A content block within an MCP tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpContentBlock {
    /// Plain text content.
    Text { text: String },
    /// Image content (base64-encoded).
    Image {
        data: String,
        #[serde(rename = "mimeType", alias = "mime_type")]
        mime_type: String,
    },
    /// Embedded resource content.
    Resource { resource: McpResourceContent },
}

/// Embedded resource content within a content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpResourceContent {
    /// The resource URI.
    pub uri: String,
    /// MIME type of the resource.
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "mimeType")]
    pub mime_type: Option<String>,
    /// The resource text content (for text resources).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

// ---------------------------------------------------------------------------
// MCP server lifecycle status
// ---------------------------------------------------------------------------

/// The lifecycle status of an MCP server connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum McpServerStatus {
    /// The server is stopped and not connected.
    Stopped,
    /// The server is starting up (launching process or connecting).
    Starting,
    /// The server is running and ready to accept requests.
    Running,
    /// The server has failed and is no longer running.
    Failed,
}

impl std::fmt::Display for McpServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

// ---------------------------------------------------------------------------
// Serde helper: serialize/deserialize JSON value as a string
// ---------------------------------------------------------------------------

/// Serde module that serializes `serde_json::Value` to/from a JSON string.
/// This allows `McpToolDef::input_schema` to be stored as `Option<String>`
/// (a JSON string) while still transmitting as a raw JSON object over the wire.
mod json_string_option {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        value: &Option<String>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(json_str) => {
                // Parse the string as JSON and serialize as a raw JSON value
                let v: serde_json::Value =
                    serde_json::from_str(json_str).unwrap_or(serde_json::Value::Null);
                serializer.serialize_some(&v)
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<String>, D::Error> {
        let opt: Option<serde_json::Value> = Option::deserialize(deserializer)?;
        Ok(opt.map(|v| serde_json::to_string(&v).unwrap_or_default()))
    }
}

// ---------------------------------------------------------------------------
// MCP connectivity test result
// ---------------------------------------------------------------------------

/// Result of a connectivity test to an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConnectivityResult {
    /// The server is reachable and returned tools.
    Connected {
        /// Number of tools discovered on the server.
        tool_count: u32,
    },
    /// The server could not be reached or the operation timed out.
    Failed {
        /// Human-readable reason for the failure.
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_mcp_server_def_stdio_roundtrip() {
        let def = McpServerDef {
            name: "my-server".to_string(),
            transport: McpTransportDef::Stdio {
                command: "npx".to_string(),
                cwd: None,
            },
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
            ],
            env: std::collections::HashMap::new(),
            keep_alive: false,
            idle_timeout_secs: 300,
            auto_restart: true,
            max_restart_attempts: 3,
        };

        // Serialize to JSON roundtrip
        let json_str = serde_json::to_string(&def).unwrap();
        let parsed: McpServerDef = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.name, "my-server");
        assert!(!parsed.keep_alive);
        assert_eq!(parsed.idle_timeout_secs, 300);
        assert!(parsed.auto_restart);
        assert_eq!(parsed.max_restart_attempts, 3);
        assert!(matches!(parsed.transport, McpTransportDef::Stdio { .. }));
        if let McpTransportDef::Stdio { command, cwd } = &parsed.transport {
            assert_eq!(command, "npx");
            assert!(cwd.is_none());
        }
        assert_eq!(
            parsed.args,
            vec!["-y", "@modelcontextprotocol/server-filesystem"]
        );

        // TOML roundtrip
        let toml_str = toml::to_string(&def).unwrap();
        let from_toml: McpServerDef = toml::from_str(&toml_str).unwrap();
        assert_eq!(from_toml.name, def.name);
        assert!(matches!(from_toml.transport, McpTransportDef::Stdio { .. }));
        if let McpTransportDef::Stdio { command, .. } = &from_toml.transport {
            assert_eq!(command, "npx");
        }
    }

    #[test]
    fn test_mcp_server_def_sse_roundtrip() {
        let def = McpServerDef {
            name: "remote-server".to_string(),
            transport: McpTransportDef::Sse {
                url: "http://localhost:8080/sse".to_string(),
                api_key_env: None,
                headers: std::collections::HashMap::new(),
            },
            args: vec![],
            env: std::collections::HashMap::new(),
            keep_alive: true,
            idle_timeout_secs: 600,
            auto_restart: false,
            max_restart_attempts: 0,
        };

        let toml_str = toml::to_string(&def).unwrap();
        let from_toml: McpServerDef = toml::from_str(&toml_str).unwrap();
        assert_eq!(from_toml.name, "remote-server");
        assert!(matches!(from_toml.transport, McpTransportDef::Sse { .. }));
        if let McpTransportDef::Sse { url, .. } = &from_toml.transport {
            assert_eq!(url, "http://localhost:8080/sse");
        }
        assert!(from_toml.keep_alive);
        assert_eq!(from_toml.idle_timeout_secs, 600);
    }

    #[test]
    fn test_mcp_tool_result_text_content() {
        let json = json!({
            "content": [
                { "type": "text", "text": "Hello, world!" }
            ]
        });

        let result: McpToolResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.content.len(), 1);
        assert!(result.is_error.is_none());
        match &result.content[0] {
            McpContentBlock::Text { text } => assert_eq!(text, "Hello, world!"),
            other => panic!("Expected Text block, got {:?}", other),
        }
    }

    #[test]
    fn test_mcp_tool_result_error() {
        let json = json!({
            "content": [
                { "type": "text", "text": "Tool execution failed" }
            ],
            "is_error": true
        });

        let result: McpToolResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_mcp_content_block_all_variants() {
        // Text variant
        let text_json = json!({ "type": "text", "text": "hello" });
        let block: McpContentBlock = serde_json::from_value(text_json).unwrap();
        assert!(matches!(block, McpContentBlock::Text { .. }));

        // Image variant
        let image_json = json!({
            "type": "image",
            "data": "iVBORw0KGgo=",
            "mime_type": "image/png"
        });
        let block: McpContentBlock = serde_json::from_value(image_json).unwrap();
        match &block {
            McpContentBlock::Image { data, mime_type } => {
                assert_eq!(data, "iVBORw0KGgo=");
                assert_eq!(mime_type, "image/png");
            }
            other => panic!("Expected Image block, got {:?}", other),
        }

        // Resource variant
        let resource_json = json!({
            "type": "resource",
            "resource": {
                "uri": "file:///tmp/readme.md",
                "mime_type": "text/markdown",
                "text": "# Hello"
            }
        });
        let block: McpContentBlock = serde_json::from_value(resource_json).unwrap();
        match &block {
            McpContentBlock::Resource { resource } => {
                assert_eq!(resource.uri, "file:///tmp/readme.md");
                assert_eq!(resource.mime_type, Some("text/markdown".to_string()));
                assert_eq!(resource.text, Some("# Hello".to_string()));
            }
            other => panic!("Expected Resource block, got {:?}", other),
        }
    }

    #[test]
    fn test_mcp_server_status_serialization() {
        // snake_case serialization
        assert_eq!(
            serde_json::to_string(&McpServerStatus::Stopped).unwrap(),
            "\"stopped\""
        );
        assert_eq!(
            serde_json::to_string(&McpServerStatus::Starting).unwrap(),
            "\"starting\""
        );
        assert_eq!(
            serde_json::to_string(&McpServerStatus::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&McpServerStatus::Failed).unwrap(),
            "\"failed\""
        );

        // Deserialization
        let status: McpServerStatus = serde_json::from_str("\"running\"").unwrap();
        assert_eq!(status, McpServerStatus::Running);

        // Display
        assert_eq!(McpServerStatus::Running.to_string(), "running");
        assert_eq!(McpServerStatus::Failed.to_string(), "failed");
    }

    #[test]
    fn test_json_rpc_request_construction() {
        let req = JsonRpcRequest::new(
            1,
            "initialize",
            Some(json!({"protocolVersion": "2024-11-05"})),
        );
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, json!(1));
        assert_eq!(req.method, "initialize");

        let req_str = JsonRpcRequest::new_string_id("abc", "tools/list", None);
        assert_eq!(req_str.id, json!("abc"));
    }

    #[test]
    fn test_mcp_tool_def_with_input_schema() {
        let json = json!({
            "name": "read_file",
            "description": "Read a file from disk",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        });

        let tool: McpToolDef = serde_json::from_value(json).unwrap();
        assert_eq!(tool.name, "read_file");
        assert!(tool.input_schema.is_some());
        // input_schema should be stored as a JSON string
        let schema_str = tool.input_schema.as_ref().unwrap().clone();
        let schema: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
        assert_eq!(schema["type"], "object");

        // Roundtrip
        let roundtrip = serde_json::to_string(&tool).unwrap();
        let parsed: McpToolDef = serde_json::from_str(&roundtrip).unwrap();
        assert_eq!(parsed.name, "read_file");
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

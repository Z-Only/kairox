//! MCP (Model Context Protocol) domain types.
//!
//! Defines MCP-specific types including:
//! - Server definition and transport config
//! - Tool, resource, and prompt discovery types
//! - Tool invocation result types
//! - Server lifecycle status
//!
//! JSON-RPC 2.0 wire-protocol types and server handshake/capability types
//! live in [`crate::protocol`].

use serde::{Deserialize, Serialize};

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
    /// Connect to a server via MCP Streamable HTTP transport (2025 spec).
    StreamableHttp {
        /// The URL of the Streamable HTTP endpoint.
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
// MCP health check + tool management
// ---------------------------------------------------------------------------

/// Result of a health check against an MCP server.
/// Success = tools were fetched (server is reachable and responsive).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CheckHealthResult {
    pub tools: Vec<McpToolDef>,
    pub healthy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Per-tool enabled/disabled state for a server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct McpToolStates {
    pub disabled_tools: Vec<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;

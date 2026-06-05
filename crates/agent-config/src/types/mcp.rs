use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP transport type for server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportType {
    Stdio,
    Sse,
    StreamableHttp,
}

pub(crate) fn default_idle_timeout() -> u64 {
    300
}

pub(crate) fn default_max_restart_attempts() -> u32 {
    3
}

/// MCP server configuration from TOML.
/// This is the TOML-facing type; it converts to agent_mcp::McpServerDef.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub r#type: McpTransportType,

    // stdio fields
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub cwd: Option<String>,

    // sse fields
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub api_key_env: Option<String>,

    // lifecycle options
    #[serde(default)]
    pub keep_alive: bool,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    #[serde(default = "crate::default_true")]
    pub auto_restart: bool,
    #[serde(default = "default_max_restart_attempts")]
    pub max_restart_attempts: u32,
}

impl McpServerConfig {
    /// Convert this TOML-facing config into an `agent_mcp::McpServerDef`.
    pub fn to_server_def(&self, id: &str) -> agent_mcp::McpServerDef {
        let transport = match self.r#type {
            McpTransportType::Stdio => agent_mcp::McpTransportDef::Stdio {
                command: self.command.clone().unwrap_or_default(),
                cwd: self.cwd.clone(),
            },
            McpTransportType::Sse => agent_mcp::McpTransportDef::Sse {
                url: self.url.clone().unwrap_or_default(),
                api_key_env: self.api_key_env.clone(),
                headers: self.headers.clone().unwrap_or_default(),
            },
            McpTransportType::StreamableHttp => agent_mcp::McpTransportDef::StreamableHttp {
                url: self.url.clone().unwrap_or_default(),
                api_key_env: self.api_key_env.clone(),
                headers: self.headers.clone().unwrap_or_default(),
            },
        };
        agent_mcp::McpServerDef {
            name: id.to_string(),
            transport,
            args: self.args.clone().unwrap_or_default(),
            env: self.env.clone().unwrap_or_default(),
            keep_alive: self.keep_alive,
            idle_timeout_secs: self.idle_timeout_secs,
            auto_restart: self.auto_restart,
            max_restart_attempts: self.max_restart_attempts,
        }
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;

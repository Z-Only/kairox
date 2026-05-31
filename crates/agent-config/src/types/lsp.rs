use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// LSP server configuration from TOML.
/// Converts to `agent_lsp::LspServerDef`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub file_patterns: Vec<String>,
    #[serde(default)]
    pub initialization_options: Option<serde_json::Value>,
    #[serde(default = "crate::default_true")]
    pub auto_start: bool,
}

impl LspServerConfig {
    pub fn to_server_def(&self, id: &str) -> agent_lsp::LspServerDef {
        agent_lsp::LspServerDef {
            name: id.to_string(),
            command: self.command.clone(),
            args: self.args.clone(),
            env: self.env.clone(),
            cwd: self.cwd.clone(),
            languages: self.languages.clone(),
            file_patterns: self.file_patterns.clone(),
            initialization_options: self.initialization_options.clone(),
        }
    }
}

/// DAP server configuration from TOML.
/// Converts to `agent_lsp::DapServerDef`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub languages: Vec<String>,
}

impl DapServerConfig {
    pub fn to_server_def(&self, id: &str) -> agent_lsp::DapServerDef {
        agent_lsp::DapServerDef {
            name: id.to_string(),
            command: self.command.clone(),
            args: self.args.clone(),
            env: self.env.clone(),
            cwd: self.cwd.clone(),
            languages: self.languages.clone(),
        }
    }
}

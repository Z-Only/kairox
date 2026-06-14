use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Definition of a single model profile, loaded from TOML or generated as default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDef {
    pub provider: String,
    pub model_id: String,
    #[serde(default)]
    pub base_url: Option<String>,
    /// Direct API key value. Takes priority over `api_key_env`.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Name of an environment variable that holds the API key.
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub output_limit: Option<u64>,
    /// Response text for the fake provider.
    #[serde(default)]
    pub response: Option<String>,
    // -- new fields --
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub top_k: Option<u32>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    /// Optional preset for client-identifying request headers.
    ///
    /// `claude_code` adds the Claude Code beta and app headers used by
    /// Anthropic-compatible gateways that gate behavior by client identity.
    #[serde(default)]
    pub client_identity: Option<String>,
    #[serde(default)]
    pub supports_tools: Option<bool>,
    #[serde(default)]
    pub supports_vision: Option<bool>,
    #[serde(default)]
    pub supports_reasoning: Option<bool>,
    #[serde(default)]
    pub extra_params: Option<toml::Value>,
    /// Enable the server-side code execution tool (Anthropic `code_execution_20250825`).
    #[serde(default)]
    pub server_tool_code_execution: Option<bool>,
    /// Enable the server-side web search tool (Anthropic `web_search_20250305`).
    #[serde(default)]
    pub server_tool_web_search: Option<bool>,
    #[serde(default = "crate::default_true")]
    pub enabled: bool,
}

/// Resolve whether a profile exposes user-selectable reasoning effort.
///
/// `supports_reasoning` remains an explicit override, but known reasoning
/// models should work out of the box so GUI model switching can surface the
/// effort picker for profiles that users configure manually.
pub fn profile_supports_reasoning(def: &ProfileDef) -> bool {
    def.supports_reasoning
        .unwrap_or_else(|| model_supports_reasoning(&def.provider, &def.model_id))
}

fn model_supports_reasoning(provider: &str, model_id: &str) -> bool {
    let provider = provider.to_ascii_lowercase();
    let model_id = model_id.to_ascii_lowercase();

    provider == "anthropic"
        && (model_id.contains("claude-opus-4")
            || model_id.contains("claude-sonnet-4")
            || model_id.contains("claude-3-7-sonnet"))
}

/// Metadata about a profile for UI display.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileInfo {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub local: bool,
    pub has_api_key: bool,
    pub supports_reasoning: bool,
    #[serde(default)]
    pub provider_display: String,
    #[serde(default)]
    pub model_display: String,
    #[serde(default)]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub supports_vision: bool,
    #[serde(default)]
    pub supports_tools: bool,
}

/// Where the configuration was loaded from.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSource {
    ProjectFile,
    UserFile,
    LocalFile,
    Defaults,
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;

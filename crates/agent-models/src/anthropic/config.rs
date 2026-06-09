//! Anthropic Messages API client.
//!
//! Supports the `/v1/messages` endpoint with SSE streaming,
//! authenticating via the `x-api-key` header.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub base_url: String,
    pub api_key_env: String,
    pub default_model: String,
    pub max_tokens: u64,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub capability_overrides: Option<crate::ModelCapabilities>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub top_k: Option<u32>,
    #[serde(default)]
    pub extra_params: Option<serde_json::Value>,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.anthropic.com".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            default_model: "claude-sonnet-4-20250514".into(),
            max_tokens: 16_384,
            headers: Vec::new(),
            capability_overrides: None,
            temperature: None,
            top_p: None,
            top_k: None,
            extra_params: None,
        }
    }
}

impl AnthropicConfig {
    pub fn capabilities(&self) -> crate::ModelCapabilities {
        self.capability_overrides
            .clone()
            .unwrap_or(crate::ModelCapabilities {
                streaming: true,
                tool_calling: true,
                json_schema: false,
                vision: false,
                reasoning_controls: false,
                context_window: 200_000,
                output_limit: self.max_tokens,
                local_model: false,
            })
    }

    pub(super) fn api_key(&self) -> Option<String> {
        std::env::var(&self.api_key_env).ok()
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;

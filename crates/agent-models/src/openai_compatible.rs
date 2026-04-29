use crate::profile::ModelCapabilities;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAiCompatibleConfig {
    pub base_url: String,
    pub api_key_env: String,
    pub default_model: String,
    pub headers: Vec<(String, String)>,
    pub capability_overrides: Option<ModelCapabilities>,
}

impl OpenAiCompatibleConfig {
    pub fn default_capabilities(&self) -> ModelCapabilities {
        self.capability_overrides
            .clone()
            .unwrap_or(ModelCapabilities {
                streaming: true,
                tool_calling: true,
                json_schema: true,
                vision: false,
                reasoning_controls: false,
                context_window: 128_000,
                output_limit: 16_384,
                local_model: false,
            })
    }
}

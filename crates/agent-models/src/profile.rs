use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub streaming: bool,
    pub tool_calling: bool,
    pub json_schema: bool,
    pub vision: bool,
    pub reasoning_controls: bool,
    pub context_window: u64,
    pub output_limit: u64,
    pub local_model: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProfile {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub capabilities: ModelCapabilities,
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;

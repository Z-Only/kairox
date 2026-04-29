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
mod tests {
    use super::*;

    #[test]
    fn profile_exposes_capabilities_without_ui_types() {
        let profile = ModelProfile {
            alias: "fast".into(),
            provider: "openai_compatible".into(),
            model_id: "gpt-4.1-mini".into(),
            capabilities: ModelCapabilities {
                streaming: true,
                tool_calling: true,
                json_schema: true,
                vision: false,
                reasoning_controls: false,
                context_window: 128_000,
                output_limit: 16_384,
                local_model: false,
            },
        };

        assert_eq!(profile.alias, "fast");
        assert!(profile.capabilities.tool_calling);
        assert!(!profile.capabilities.local_model);
    }
}

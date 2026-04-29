use crate::profile::ModelCapabilities;

pub fn ollama_default_capabilities(context_window: u64) -> ModelCapabilities {
    ModelCapabilities {
        streaming: true,
        tool_calling: false,
        json_schema: false,
        vision: false,
        reasoning_controls: false,
        context_window,
        output_limit: 4096,
        local_model: true,
    }
}

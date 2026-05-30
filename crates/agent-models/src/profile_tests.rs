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

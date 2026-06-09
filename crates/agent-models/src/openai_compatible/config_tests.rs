use super::*;

#[test]
fn default_capabilities_without_overrides() {
    let config = OpenAiCompatibleConfig {
        base_url: "https://api.example.com".into(),
        api_key_env: "TEST_KEY".into(),
        default_model: "gpt-4".into(),
        headers: Vec::new(),
        capability_overrides: None,
        temperature: None,
        top_p: None,
        extra_params: None,
    };
    let caps = config.default_capabilities();
    assert!(caps.streaming);
    assert!(caps.tool_calling);
    assert!(caps.json_schema);
    assert!(!caps.vision);
    assert!(!caps.reasoning_controls);
    assert_eq!(caps.context_window, 128_000);
    assert_eq!(caps.output_limit, 16_384);
    assert!(!caps.local_model);
}

#[test]
fn default_capabilities_respects_overrides() {
    let overrides = crate::ModelCapabilities {
        streaming: false,
        tool_calling: false,
        json_schema: false,
        vision: true,
        reasoning_controls: true,
        context_window: 32_000,
        output_limit: 2_048,
        local_model: true,
    };
    let config = OpenAiCompatibleConfig {
        base_url: "https://api.example.com".into(),
        api_key_env: "TEST_KEY".into(),
        default_model: "custom-model".into(),
        headers: Vec::new(),
        capability_overrides: Some(overrides.clone()),
        temperature: None,
        top_p: None,
        extra_params: None,
    };
    assert_eq!(config.default_capabilities(), overrides);
}

#[test]
fn api_key_reads_from_env() {
    let unique_var = "KAIROX_TEST_OAI_KEY_CONFIG_7412";
    let config = OpenAiCompatibleConfig {
        base_url: "https://api.example.com".into(),
        api_key_env: unique_var.to_string(),
        default_model: "gpt-4".into(),
        headers: Vec::new(),
        capability_overrides: None,
        temperature: None,
        top_p: None,
        extra_params: None,
    };

    std::env::remove_var(unique_var);
    assert!(config.api_key().is_none());

    std::env::set_var(unique_var, "sk-openai-test");
    assert_eq!(config.api_key().unwrap(), "sk-openai-test");

    std::env::remove_var(unique_var);
}

#[test]
fn serde_round_trip() {
    let config = OpenAiCompatibleConfig {
        base_url: "https://custom.api.com/v1".into(),
        api_key_env: "MY_KEY".into(),
        default_model: "gpt-4o".into(),
        headers: vec![("X-Custom".into(), "value".into())],
        capability_overrides: None,
        temperature: Some(0.8),
        top_p: Some(0.95),
        extra_params: Some(serde_json::json!({"frequency_penalty": 0.5})),
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: OpenAiCompatibleConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config, deserialized);
}

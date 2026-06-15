use super::*;

#[test]
fn default_config_has_expected_values() {
    let config = AnthropicConfig::default();
    assert_eq!(config.base_url, "https://api.anthropic.com");
    assert_eq!(config.api_key_env, "ANTHROPIC_API_KEY");
    assert_eq!(config.default_model, "claude-sonnet-4-20250514");
    assert_eq!(config.max_tokens, 16_384);
    assert_eq!(config.connect_timeout_secs, 15);
    assert!(config.request_timeout_secs.is_none());
    assert!(config.headers.is_empty());
    assert!(config.capability_overrides.is_none());
    assert!(config.temperature.is_none());
    assert!(config.top_p.is_none());
    assert!(config.top_k.is_none());
    assert!(config.extra_params.is_none());
}

#[test]
fn capabilities_returns_defaults_when_no_overrides() {
    let config = AnthropicConfig::default();
    let caps = config.capabilities();
    assert!(caps.streaming);
    assert!(caps.tool_calling);
    assert!(!caps.json_schema);
    assert!(!caps.vision);
    assert!(!caps.reasoning_controls);
    assert_eq!(caps.context_window, 200_000);
    assert_eq!(caps.output_limit, config.max_tokens);
    assert!(!caps.local_model);
}

#[test]
fn capabilities_respects_overrides() {
    let overrides = crate::ModelCapabilities {
        streaming: false,
        tool_calling: false,
        json_schema: true,
        vision: true,
        reasoning_controls: true,
        context_window: 50_000,
        output_limit: 4_096,
        local_model: true,
    };
    let config = AnthropicConfig {
        capability_overrides: Some(overrides.clone()),
        ..AnthropicConfig::default()
    };
    assert_eq!(config.capabilities(), overrides);
}

#[test]
fn api_key_reads_from_env() {
    let unique_var = "KAIROX_TEST_ANTHROPIC_KEY_CONFIG_9283";
    let config = AnthropicConfig {
        api_key_env: unique_var.to_string(),
        ..AnthropicConfig::default()
    };

    // Not set → None
    std::env::remove_var(unique_var);
    assert!(config.api_key().is_none());

    // Set → Some
    std::env::set_var(unique_var, "sk-test-key");
    assert_eq!(config.api_key().unwrap(), "sk-test-key");

    std::env::remove_var(unique_var);
}

#[test]
fn serde_round_trip() {
    let config = AnthropicConfig {
        temperature: Some(0.7),
        top_k: Some(40),
        connect_timeout_secs: 10,
        request_timeout_secs: Some(900),
        extra_params: Some(serde_json::json!({"foo": "bar"})),
        ..AnthropicConfig::default()
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: AnthropicConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config, deserialized);
}

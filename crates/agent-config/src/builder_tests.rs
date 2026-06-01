use super::*;
use agent_models::ModelClient;
use futures::StreamExt;

#[test]
fn build_router_registers_all_profiles() {
    let config = Config::defaults();
    let router = build_router(&config);
    let profiles = router.list_profiles();
    assert!(!profiles.is_empty());
    assert!(profiles.iter().any(|p| p.alias == "fake"));
    // local-code is disabled by default and should be excluded.
    assert!(!profiles.iter().any(|p| p.alias == "local-code"));
}

#[tokio::test]
async fn fake_profile_produces_tokens() {
    let config = Config::defaults();
    let router = build_router(&config);

    let mut stream = router
        .stream(agent_models::ModelRequest::user_text("fake", "hello"))
        .await
        .unwrap();

    let mut tokens = Vec::new();
    while let Some(event) = stream.next().await {
        match event {
            Ok(agent_models::ModelEvent::TokenDelta(d)) => tokens.push(d),
            Ok(agent_models::ModelEvent::Completed { .. }) => break,
            _ => {}
        }
    }

    assert!(!tokens.is_empty());
}

#[tokio::test]
async fn unknown_profile_returns_error() {
    let config = Config::defaults();
    let router = build_router(&config);

    let result = router
        .stream(agent_models::ModelRequest::user_text(
            "nonexistent",
            "hello",
        ))
        .await;

    assert!(result.is_err());
}

#[test]
fn build_profile_sets_capabilities_per_provider() {
    let fast_def = ProfileDef {
        provider: "anthropic".into(),
        model_id: "claude-sonnet-4-20250514".into(),
        base_url: Some("https://api.anthropic.com".into()),
        api_key: None,
        api_key_env: Some("ANTHROPIC_API_KEY".into()),
        context_window: Some(200_000),
        output_limit: Some(16_384),
        response: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        top_k: None,
        headers: None,
        supports_tools: None,
        supports_vision: None,
        supports_reasoning: None,
        extra_params: None,
        server_tool_code_execution: None,
        server_tool_web_search: None,
        enabled: true,
    };
    let profile = build_profile("fast", &fast_def);
    assert_eq!(profile.alias, "fast");
    assert!(profile.capabilities.tool_calling);
    assert!(profile.capabilities.reasoning_controls);
    assert!(!profile.capabilities.local_model);

    let ollama_def = ProfileDef {
        provider: "ollama".into(),
        model_id: "devstral".into(),
        base_url: Some("http://localhost:11434".into()),
        api_key: None,
        api_key_env: None,
        context_window: Some(128_000),
        output_limit: Some(16_384),
        response: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        top_k: None,
        headers: None,
        supports_tools: None,
        supports_vision: None,
        supports_reasoning: None,
        extra_params: None,
        server_tool_code_execution: None,
        server_tool_web_search: None,
        enabled: true,
    };
    let profile = build_profile("local-code", &ollama_def);
    assert!(!profile.capabilities.tool_calling);
    assert!(profile.capabilities.local_model);
}

#[test]
fn provider_family_maps_correctly() {
    assert_eq!(
        provider_family(&test_profile("anthropic", None)),
        "anthropic"
    );
    assert_eq!(provider_family(&test_profile("ollama", None)), "ollama");
    assert_eq!(provider_family(&test_profile("fake", None)), "fake");
    assert_eq!(
        provider_family(&test_profile("openai_compatible", None)),
        "openai_compatible"
    );
    assert_eq!(
        provider_family(&test_profile("deepseek", None)),
        "openai_compatible"
    );
    assert_eq!(
        provider_family(&test_profile("groq", None)),
        "openai_compatible"
    );
    assert_eq!(
        provider_family(&test_profile("xai", None)),
        "openai_compatible"
    );
    assert_eq!(
        provider_family(&test_profile("unknown-thing", None)),
        "openai_compatible"
    );
}

#[test]
fn custom_provider_with_anthropic_base_url_uses_anthropic_family() {
    let def = test_profile("ali-mo", Some("https://idealab.example.com/api/anthropic"));

    assert_eq!(provider_family(&def), "anthropic");
}

#[test]
fn custom_anthropic_gateway_profile_uses_anthropic_capabilities() {
    let def = test_profile("ali-mo", Some("https://idealab.example.com/api/anthropic"));
    let profile = build_profile("ali-mo-claude", &def);

    assert_eq!(profile.provider, "ali-mo");
    assert!(profile.capabilities.tool_calling);
    assert!(!profile.capabilities.json_schema);
    assert!(profile.capabilities.reasoning_controls);
}

#[test]
fn capability_overrides_from_profile_def() {
    let def = ProfileDef {
        provider: "deepseek".into(),
        model_id: "deepseek-chat".into(),
        base_url: Some("https://api.deepseek.com/v1".into()),
        api_key: None,
        api_key_env: Some("DEEPSEEK_API_KEY".into()),
        context_window: Some(128_000),
        output_limit: Some(32_768),
        response: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        top_k: None,
        headers: None,
        supports_tools: Some(false),
        supports_vision: Some(true),
        supports_reasoning: None,
        extra_params: None,
        server_tool_code_execution: None,
        server_tool_web_search: None,
        enabled: true,
    };
    let profile = build_profile("deepseek", &def);
    // Overridden
    assert!(!profile.capabilities.tool_calling);
    assert!(profile.capabilities.vision);
    // Not overridden -- uses provider default (openai_compatible defaults)
    assert!(!profile.capabilities.reasoning_controls);
}

#[test]
fn deepseek_profile_builds_as_openai_compatible_client() {
    let toml = r#"
[profiles.deepseek]
provider = "deepseek"
model_id = "deepseek-chat"
base_url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"
temperature = 0.6
top_p = 0.95

[profiles.deepseek.extra_params]
frequency_penalty = 0.1
"#;
    let config = crate::loader::load_from_str(toml, "test.toml").unwrap();
    let router = build_router(&config);
    let profiles = router.list_profiles();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].alias, "deepseek");
    assert_eq!(profiles[0].provider, "deepseek");
    // Should have openai_compatible default capabilities (tool_calling = true)
    assert!(profiles[0].capabilities.tool_calling);
}

fn test_profile(provider: &str, base_url: Option<&str>) -> ProfileDef {
    ProfileDef {
        provider: provider.into(),
        model_id: "claude-opus-4-6".into(),
        base_url: base_url.map(str::to_string),
        api_key: None,
        api_key_env: None,
        context_window: Some(200_000),
        output_limit: Some(16_384),
        response: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        top_k: None,
        headers: None,
        supports_tools: None,
        supports_vision: None,
        supports_reasoning: None,
        extra_params: None,
        server_tool_code_execution: None,
        server_tool_web_search: None,
        enabled: true,
    }
}

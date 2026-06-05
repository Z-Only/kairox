use super::*;

// ── ProfileDef serde ────────────────────────────────────────────────

#[test]
fn profile_def_minimal_toml() {
    let toml_str = r#"
        provider = "anthropic"
        model_id = "claude-sonnet-4-20250514"
    "#;
    let def: ProfileDef = toml::from_str(toml_str).expect("minimal profile parses");
    assert_eq!(def.provider, "anthropic");
    assert_eq!(def.model_id, "claude-sonnet-4-20250514");
    assert!(def.base_url.is_none());
    assert!(def.api_key.is_none());
    assert!(def.api_key_env.is_none());
    assert!(def.context_window.is_none());
    assert!(def.output_limit.is_none());
    assert!(def.max_tokens.is_none());
    assert!(def.temperature.is_none());
    assert!(def.top_p.is_none());
    assert!(def.top_k.is_none());
    assert!(def.headers.is_none());
    assert!(def.client_identity.is_none());
    assert!(def.supports_tools.is_none());
    assert!(def.supports_vision.is_none());
    assert!(def.supports_reasoning.is_none());
    assert!(def.extra_params.is_none());
    assert!(def.server_tool_code_execution.is_none());
    assert!(def.server_tool_web_search.is_none());
    assert!(def.enabled, "enabled defaults to true");
}

#[test]
fn profile_def_full_toml() {
    let toml_str = r#"
        provider = "openai"
        model_id = "gpt-4o"
        base_url = "https://api.example.com"
        api_key = "sk-secret"
        api_key_env = "OPENAI_API_KEY"
        context_window = 128000
        output_limit = 4096
        max_tokens = 2048
        temperature = 0.7
        top_p = 0.9
        top_k = 40
        client_identity = "claude_code"
        supports_tools = true
        supports_vision = false
        supports_reasoning = true
        enabled = false
        server_tool_code_execution = true
        server_tool_web_search = false

        [headers]
        X-Custom = "value"
    "#;
    let def: ProfileDef = toml::from_str(toml_str).expect("full profile parses");
    assert_eq!(def.provider, "openai");
    assert_eq!(def.model_id, "gpt-4o");
    assert_eq!(def.base_url.as_deref(), Some("https://api.example.com"));
    assert_eq!(def.api_key.as_deref(), Some("sk-secret"));
    assert_eq!(def.api_key_env.as_deref(), Some("OPENAI_API_KEY"));
    assert_eq!(def.context_window, Some(128000));
    assert_eq!(def.output_limit, Some(4096));
    assert_eq!(def.max_tokens, Some(2048));
    assert!((def.temperature.unwrap() - 0.7).abs() < f32::EPSILON);
    assert!((def.top_p.unwrap() - 0.9).abs() < f32::EPSILON);
    assert_eq!(def.top_k, Some(40));
    assert_eq!(def.client_identity.as_deref(), Some("claude_code"));
    assert_eq!(def.supports_tools, Some(true));
    assert_eq!(def.supports_vision, Some(false));
    assert_eq!(def.supports_reasoning, Some(true));
    assert!(!def.enabled);
    assert_eq!(def.server_tool_code_execution, Some(true));
    assert_eq!(def.server_tool_web_search, Some(false));
    let hdrs = def.headers.as_ref().expect("headers present");
    assert_eq!(hdrs.get("X-Custom").map(|s| s.as_str()), Some("value"));
}

#[test]
fn profile_def_roundtrip_json() {
    let def = ProfileDef {
        provider: "fake".into(),
        model_id: "test".into(),
        base_url: None,
        api_key: None,
        api_key_env: Some("KEY".into()),
        context_window: Some(8000),
        output_limit: None,
        response: Some("hello".into()),
        max_tokens: None,
        temperature: Some(0.5),
        top_p: None,
        top_k: None,
        headers: None,
        client_identity: None,
        supports_tools: None,
        supports_vision: None,
        supports_reasoning: None,
        extra_params: None,
        server_tool_code_execution: None,
        server_tool_web_search: None,
        enabled: true,
    };
    let json = serde_json::to_string(&def).expect("serialize");
    let back: ProfileDef = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.provider, "fake");
    assert_eq!(back.api_key_env.as_deref(), Some("KEY"));
    assert_eq!(back.context_window, Some(8000));
    assert_eq!(back.response.as_deref(), Some("hello"));
    assert!((back.temperature.unwrap() - 0.5).abs() < f32::EPSILON);
    assert!(back.enabled);
}

// ── profile_supports_reasoning ──────────────────────────────────────

fn make_profile(provider: &str, model_id: &str) -> ProfileDef {
    ProfileDef {
        provider: provider.into(),
        model_id: model_id.into(),
        base_url: None,
        api_key: None,
        api_key_env: None,
        context_window: None,
        output_limit: None,
        response: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        top_k: None,
        headers: None,
        client_identity: None,
        supports_tools: None,
        supports_vision: None,
        supports_reasoning: None,
        extra_params: None,
        server_tool_code_execution: None,
        server_tool_web_search: None,
        enabled: true,
    }
}

#[test]
fn explicit_supports_reasoning_overrides_auto_detection() {
    let mut def = make_profile("openai", "gpt-4o");
    def.supports_reasoning = Some(true);
    assert!(profile_supports_reasoning(&def));

    let mut def2 = make_profile("anthropic", "claude-opus-4-20250514");
    def2.supports_reasoning = Some(false);
    assert!(!profile_supports_reasoning(&def2));
}

#[test]
fn auto_detect_anthropic_reasoning_models() {
    let opus = make_profile("anthropic", "claude-opus-4-20250514");
    assert!(profile_supports_reasoning(&opus));

    let sonnet4 = make_profile("anthropic", "claude-sonnet-4-20250514");
    assert!(profile_supports_reasoning(&sonnet4));

    let sonnet37 = make_profile("anthropic", "claude-3-7-sonnet-20250219");
    assert!(profile_supports_reasoning(&sonnet37));

    // Case insensitive
    let upper = make_profile("Anthropic", "Claude-Opus-4-Latest");
    assert!(profile_supports_reasoning(&upper));
}

#[test]
fn non_reasoning_models_return_false() {
    let haiku = make_profile("anthropic", "claude-3-5-haiku-20241022");
    assert!(!profile_supports_reasoning(&haiku));

    let openai = make_profile("openai", "gpt-4o");
    assert!(!profile_supports_reasoning(&openai));

    let fake = make_profile("fake", "fake-model");
    assert!(!profile_supports_reasoning(&fake));
}

// ── ProfileInfo ─────────────────────────────────────────────────────

#[test]
fn profile_info_serde_roundtrip() {
    let info = ProfileInfo {
        alias: "default".into(),
        provider: "anthropic".into(),
        model_id: "claude-sonnet-4".into(),
        local: false,
        has_api_key: true,
        supports_reasoning: true,
        provider_display: "Anthropic".into(),
        model_display: "Claude Sonnet 4".into(),
    };
    let json = serde_json::to_string(&info).expect("serialize ProfileInfo");
    let back: ProfileInfo = serde_json::from_str(&json).expect("deserialize ProfileInfo");
    assert_eq!(back.alias, "default");
    assert_eq!(back.provider_display, "Anthropic");
    assert_eq!(back.model_display, "Claude Sonnet 4");
    assert!(back.has_api_key);
    assert!(back.supports_reasoning);
    assert!(!back.local);
}

// ── ConfigSource ────────────────────────────────────────────────────

#[test]
fn config_source_equality() {
    assert_eq!(ConfigSource::ProjectFile, ConfigSource::ProjectFile);
    assert_eq!(ConfigSource::UserFile, ConfigSource::UserFile);
    assert_eq!(ConfigSource::LocalFile, ConfigSource::LocalFile);
    assert_eq!(ConfigSource::Defaults, ConfigSource::Defaults);
    assert_ne!(ConfigSource::ProjectFile, ConfigSource::UserFile);
    assert_ne!(ConfigSource::Defaults, ConfigSource::LocalFile);
}

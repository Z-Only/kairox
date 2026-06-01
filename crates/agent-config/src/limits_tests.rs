use super::*;

fn profile(provider: &str, model_id: &str, ctx: Option<u64>, out: Option<u64>) -> ProfileDef {
    ProfileDef {
        provider: provider.into(),
        model_id: model_id.into(),
        base_url: None,
        api_key: None,
        api_key_env: None,
        context_window: ctx,
        output_limit: out,
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
fn user_config_wins_when_both_fields_set() {
    let limits = resolve_limits(&profile(
        "anthropic",
        "claude-sonnet-4",
        Some(50_000),
        Some(4_000),
    ));
    assert_eq!(limits.context_window, 50_000);
    assert_eq!(limits.output_limit, 4_000);
    assert_eq!(limits.source, LimitSource::UserConfig);
}

#[test]
fn builtin_registry_used_when_user_omits_both_fields() {
    let limits = resolve_limits(&profile("anthropic", "claude-sonnet-4", None, None));
    assert_eq!(limits.context_window, 200_000);
    assert_eq!(limits.output_limit, 8_192);
    assert_eq!(limits.source, LimitSource::BuiltinRegistry);
}

#[test]
fn partial_override_keeps_other_field_from_registry() {
    let limits = resolve_limits(&profile("openai_compatible", "gpt-4o", Some(64_000), None));
    assert_eq!(limits.context_window, 64_000);
    assert_eq!(limits.output_limit, 16_384); // from registry
    assert_eq!(limits.source, LimitSource::UserConfig);
}

#[test]
fn unknown_model_falls_back_to_provider_default() {
    let limits = resolve_limits(&profile("ollama", "weird-model", None, None));
    assert_eq!(limits.context_window, 8_192);
    assert_eq!(limits.source, LimitSource::Fallback);
}

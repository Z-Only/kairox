use super::*;

#[test]
fn looks_up_gpt_4o_with_builtin_source() {
    let limits = lookup("openai_compatible", "gpt-4o");
    assert_eq!(limits.context_window, 128_000);
    assert_eq!(limits.output_limit, 16_384);
    assert_eq!(limits.source, LimitSource::BuiltinRegistry);
}

#[test]
fn longest_prefix_wins_for_overlapping_anthropic_patterns() {
    let limits = lookup("anthropic", "claude-3-5-sonnet-20240620");
    assert_eq!(limits.context_window, 200_000);
    assert_eq!(limits.output_limit, 8_192);
    assert_eq!(limits.source, LimitSource::BuiltinRegistry);
}

#[test]
fn returns_provider_fallback_for_unknown_openai_model() {
    let limits = lookup("openai_compatible", "gpt-future-9000");
    assert_eq!(limits.source, LimitSource::Fallback);
    assert_eq!(limits.context_window, 128_000);
}

#[test]
fn ollama_provider_always_returns_conservative_fallback() {
    let limits = lookup("ollama", "llama3:70b");
    assert_eq!(limits.context_window, 8_192);
    assert_eq!(limits.source, LimitSource::Fallback);
}

#[test]
fn fake_provider_returns_small_window() {
    let limits = lookup("fake", "fake");
    assert_eq!(limits.context_window, 4_096);
    assert_eq!(limits.source, LimitSource::Fallback);
}

#[test]
fn unknown_provider_returns_generic_fallback() {
    let limits = lookup("custom", "anything");
    assert_eq!(limits.source, LimitSource::Fallback);
    assert_eq!(limits.context_window, 128_000);
}

#[test]
fn lookup_openai_gpt41_returns_builtin() {
    let limits = lookup("openai_compatible", "gpt-4.1");
    assert_eq!(limits.source, LimitSource::BuiltinRegistry);
    assert_eq!(limits.context_window, 1_048_576);
    assert_eq!(limits.output_limit, 32_768);
}

#[test]
fn lookup_anthropic_claude_sonnet_4() {
    let limits = lookup("anthropic", "claude-sonnet-4-20250514");
    assert_eq!(limits.source, LimitSource::BuiltinRegistry);
    assert_eq!(limits.context_window, 200_000);
}

#[test]
fn lookup_anthropic_unknown_returns_fallback() {
    let limits = lookup("anthropic", "unknown-model");
    assert_eq!(limits.source, LimitSource::Fallback);
    assert_eq!(limits.context_window, 128_000);
    assert_eq!(limits.output_limit, 16_384);
}

#[test]
fn lookup_claude_3_5_sonnet_suffix_variant_matches() {
    let limits = lookup("anthropic", "claude-3-5-sonnet-20241022");
    assert_eq!(limits.source, LimitSource::BuiltinRegistry);
    assert_eq!(limits.context_window, 200_000);
    assert_eq!(limits.output_limit, 8_192);
}

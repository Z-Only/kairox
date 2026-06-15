use super::*;

// ── AdvisorConfig ───────────────────────────────────────────────────

#[test]
fn advisor_config_default_is_off() {
    let config = AdvisorConfig::default();
    assert_eq!(config.mode, agent_core::AdvisorMode::Off);
    assert!(config.profile.is_none());
    assert_eq!(config.max_concerns, 5);
}

#[test]
fn advisor_config_is_default_returns_true_for_defaults() {
    assert!(AdvisorConfig::default().is_default());
}

#[test]
fn advisor_config_is_default_returns_false_when_mode_changed() {
    let config = AdvisorConfig {
        mode: agent_core::AdvisorMode::Full,
        ..AdvisorConfig::default()
    };
    assert!(!config.is_default());
}

#[test]
fn advisor_config_is_default_returns_false_when_profile_set() {
    let config = AdvisorConfig {
        profile: Some("haiku".into()),
        ..AdvisorConfig::default()
    };
    assert!(!config.is_default());
}

#[test]
fn advisor_config_is_default_returns_false_when_max_concerns_changed() {
    let config = AdvisorConfig {
        max_concerns: 10,
        ..AdvisorConfig::default()
    };
    assert!(!config.is_default());
}

#[test]
fn advisor_config_deserializes_from_toml() {
    let toml_str = r#"
        mode = "lightweight"
        profile = "haiku"
        max_concerns = 3
    "#;
    let config: AdvisorConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.mode, agent_core::AdvisorMode::Lightweight);
    assert_eq!(config.profile.as_deref(), Some("haiku"));
    assert_eq!(config.max_concerns, 3);
}

#[test]
fn advisor_config_deserializes_empty_toml_as_defaults() {
    let config: AdvisorConfig = toml::from_str("").unwrap();
    assert!(config.is_default());
}

// ── FeatureFlags ────────────────────────────────────────────────────

#[test]
fn feature_flags_default_has_hooks_enabled() {
    let flags = FeatureFlags::default();
    assert!(flags.hooks);
}

#[test]
fn feature_flags_deserializes_hooks_false() {
    let toml_str = r#"hooks = false"#;
    let flags: FeatureFlags = toml::from_str(toml_str).unwrap();
    assert!(!flags.hooks);
}

#[test]
fn feature_flags_deserializes_empty_as_defaults() {
    let flags: FeatureFlags = toml::from_str("").unwrap();
    assert!(flags.hooks);
}

// ── ContextPolicy ───────────────────────────────────────────────────

#[test]
fn context_policy_default_threshold() {
    let policy = ContextPolicy::default();
    assert!((policy.auto_compact_threshold - 0.85).abs() < f32::EPSILON);
    assert!(policy.compactor_profile.is_none());
    assert!(policy.max_tool_definition_tokens.is_none());
    assert!(policy.max_iterations.is_none());
    assert_eq!(policy.model_stream_idle_timeout_secs, Some(90));
}

#[test]
fn context_policy_deserializes_from_toml() {
    let toml_str = r#"
        auto_compact_threshold = 0.7
        compactor_profile = "summarizer"
        max_tool_definition_tokens = 4096
        max_iterations = 50
        model_stream_idle_timeout_secs = 120
    "#;
    let policy: ContextPolicy = toml::from_str(toml_str).unwrap();
    assert!((policy.auto_compact_threshold - 0.7).abs() < f32::EPSILON);
    assert_eq!(policy.compactor_profile.as_deref(), Some("summarizer"));
    assert_eq!(policy.max_tool_definition_tokens, Some(4096));
    assert_eq!(policy.max_iterations, Some(50));
    assert_eq!(policy.model_stream_idle_timeout_secs, Some(120));
}

#[test]
fn context_policy_deserializes_empty_as_defaults() {
    let policy: ContextPolicy = toml::from_str("").unwrap();
    assert!((policy.auto_compact_threshold - 0.85).abs() < f32::EPSILON);
    assert!(policy.compactor_profile.is_none());
    assert!(policy.max_tool_definition_tokens.is_none());
    assert!(policy.max_iterations.is_none());
    assert_eq!(policy.model_stream_idle_timeout_secs, Some(90));
}

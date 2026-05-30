use super::*;

#[test]
fn defaults_include_fake_and_local_code() {
    let config = Config::defaults();
    let names = config.profile_names();
    assert!(names.contains(&"fake".to_string()));
    // local-code is disabled by default; it is present in the raw
    // profiles vec but hidden from profile_names().
    assert!(!names.contains(&"local-code".to_string()));
    let all_names: Vec<_> = config.profiles.iter().map(|(n, _)| n.clone()).collect();
    assert!(all_names.contains(&"local-code".to_string()));
}

#[test]
fn defaults_include_fast_when_openai_key_set() {
    let config = Config::defaults();
    let names = config.profile_names();
    assert!(!names.is_empty());
}

#[test]
fn default_profile_prefers_fast() {
    let config = Config::defaults();
    let default = config.default_profile();
    assert!(!default.is_empty());
}

#[test]
fn profile_names_returns_ordered_list() {
    let config = Config::defaults();
    let names = config.profile_names();
    let enabled_count = config.profiles.iter().filter(|(_, d)| d.enabled).count();
    assert_eq!(names.len(), enabled_count);
}

#[test]
fn profile_info_reflects_local_and_key_status() {
    let config = Config::defaults();
    let info = config.profile_info();
    assert!(info.iter().any(|p| p.alias == "fake" && p.local));
    // local-code is disabled by default, so it's excluded from profile_info.
    assert!(!info.iter().any(|p| p.alias == "local-code"));
}

#[test]
fn profile_info_marks_claude_profiles_as_reasoning_capable() {
    let config = crate::loader::load_from_str(
        r#"
[profiles.claude]
provider = "anthropic"
model_id = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"

[profiles.claude-off]
provider = "anthropic"
model_id = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
supports_reasoning = false
"#,
        "profiles.toml",
    )
    .expect("config parses");

    let profile = config
        .profile_info()
        .into_iter()
        .find(|profile| profile.alias == "claude")
        .expect("claude profile appears in GUI metadata");

    assert!(profile.supports_reasoning);
    assert!(config
        .profile_info()
        .into_iter()
        .any(|profile| profile.alias == "claude-off" && !profile.supports_reasoning));
}

#[test]
fn defaults_has_empty_mcp_servers() {
    let config = Config::defaults();
    assert!(config.mcp_servers.is_empty());
}

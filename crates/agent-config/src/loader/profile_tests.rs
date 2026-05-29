use super::*;
use crate::loader::load_from_str;

#[test]
fn parses_valid_toml_with_multiple_profiles() {
    let toml = r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4.1-mini"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
context_window = 128_000
output_limit = 16_384

[profiles.local-code]
provider = "ollama"
model_id = "devstral"
base_url = "http://localhost:11434"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    assert_eq!(config.profiles.len(), 2);

    let (fast_name, fast_def) = &config.profiles[0];
    assert_eq!(fast_name, "fast");
    assert_eq!(fast_def.provider, "openai_compatible");
    assert_eq!(fast_def.model_id, "gpt-4.1-mini");
    assert_eq!(
        fast_def.base_url,
        Some("https://api.openai.com/v1".to_string())
    );
    assert_eq!(fast_def.api_key_env, Some("OPENAI_API_KEY".to_string()));
    assert_eq!(fast_def.context_window, Some(128_000));

    let (local_name, local_def) = &config.profiles[1];
    assert_eq!(local_name, "local-code");
    assert_eq!(local_def.provider, "ollama");
    assert_eq!(local_def.model_id, "devstral");
    assert_eq!(local_def.context_window, None);
}

#[test]
fn parses_fake_provider_with_response() {
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"
response = "hello from Kairox"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    let (_, fake_def) = &config.profiles[0];
    assert_eq!(fake_def.provider, "fake");
    assert_eq!(fake_def.response, Some("hello from Kairox".to_string()));
}

#[test]
fn accepts_any_provider_name() {
    let toml = r#"
[profiles.custom]
provider = "deepseek"
model_id = "deepseek-chat"
base_url = "https://api.deepseek.com/v1"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    let result = validate(&config);
    assert!(result.is_ok(), "any provider name should be accepted");
}

#[test]
fn rejects_openai_compatible_without_base_url() {
    let toml = r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4.1-mini"
api_key_env = "OPENAI_API_KEY"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    let result = validate(&config);
    assert!(result.is_err());
}

#[test]
fn validate_rejects_openai_compatible_without_base_url() {
    let toml = r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4.1-mini"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    let result = validate(&config);
    assert!(result.is_err());
}

#[test]
fn validate_allows_ollama_without_base_url() {
    let toml = r#"
[profiles.local-llm]
provider = "ollama"
model_id = "llama3"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    let result = validate(&config);
    assert!(result.is_ok());
}

#[test]
fn config_parse_disabled_profile_excluded() {
    let toml = r#"
[profiles.enabled-one]
provider = "fake"
model_id = "fake"

[profiles.disabled-one]
provider = "ollama"
model_id = "llama3"
enabled = false

[profiles.enabled-two]
provider = "openai_compatible"
model_id = "gpt-4"
base_url = "https://api.openai.com/v1"
"#;
    let config = load_from_str(toml, "test.toml").unwrap();

    let names = config.profile_names();
    assert!(names.contains(&"enabled-one".to_string()));
    assert!(names.contains(&"enabled-two".to_string()));
    assert!(!names.contains(&"disabled-one".to_string()));
    assert_eq!(names.len(), 2);

    let info = config.profile_info();
    assert!(info.iter().any(|p| p.alias == "enabled-one"));
    assert!(info.iter().any(|p| p.alias == "enabled-two"));
    assert!(!info.iter().any(|p| p.alias == "disabled-one"));
    assert_eq!(info.len(), 2);
}

#[test]
fn config_parse_profile_with_all_optional_fields() {
    let toml = r#"
[profiles.full]
provider = "openai_compatible"
model_id = "gpt-4"
base_url = "https://api.openai.com/v1"
temperature = 0.7
top_p = 0.9
top_k = 50
supports_tools = true
supports_vision = false
supports_reasoning = true

[profiles.full.headers]
X-Custom = "custom-value"
Authorization = "Bearer test"

[profiles.full.extra_params]
seed = 42
response_format = { type = "json_object" }
"#;
    let config = load_from_str(toml, "test.toml").unwrap();
    assert_eq!(config.profiles.len(), 1);
    let (alias, def) = &config.profiles[0];
    assert_eq!(alias, "full");
    assert_eq!(def.provider, "openai_compatible");
    assert_eq!(def.model_id, "gpt-4");
    assert!((def.temperature.unwrap() - 0.7).abs() < 1e-6);
    assert!((def.top_p.unwrap() - 0.9).abs() < 1e-6);
    assert_eq!(def.top_k, Some(50));
    assert_eq!(def.supports_tools, Some(true));
    assert_eq!(def.supports_vision, Some(false));
    assert_eq!(def.supports_reasoning, Some(true));

    let headers = def.headers.as_ref().unwrap();
    assert_eq!(headers.get("X-Custom"), Some(&"custom-value".to_string()));
    assert_eq!(
        headers.get("Authorization"),
        Some(&"Bearer test".to_string())
    );

    let extra = def.extra_params.as_ref().unwrap();
    assert_eq!(extra.get("seed").and_then(|v| v.as_integer()), Some(42));
    assert_eq!(
        extra
            .get("response_format")
            .and_then(|v| v.get("type"))
            .and_then(|v| v.as_str()),
        Some("json_object")
    );
}

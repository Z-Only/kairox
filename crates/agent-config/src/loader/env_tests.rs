use super::*;
use crate::loader::load_from_str;

#[test]
fn api_key_direct_takes_priority_over_env() {
    let toml = r#"
[profiles.test]
provider = "openai_compatible"
model_id = "test-model"
base_url = "https://api.example.com/v1"
api_key = "sk-direct-key"
api_key_env = "OPENAI_API_KEY"
"#;
    let mut config = load_from_str(toml, "test.toml").unwrap();
    resolve_api_keys(&mut config);
    let (_, def) = &config.profiles[0];
    assert_eq!(def.api_key, Some("sk-direct-key".to_string()));
}

#[test]
fn api_key_env_resolves_from_environment() {
    let toml = r#"
[profiles.test]
provider = "openai_compatible"
model_id = "test-model"
base_url = "https://api.example.com/v1"
api_key_env = "KAIROX_TEST_KEY_VAR"
"#;
    std::env::set_var("KAIROX_TEST_KEY_VAR", "sk-from-env");
    let mut config = load_from_str(toml, "test.toml").unwrap();
    resolve_api_keys(&mut config);
    let (_, def) = &config.profiles[0];
    assert_eq!(def.api_key, Some("sk-from-env".to_string()));
    std::env::remove_var("KAIROX_TEST_KEY_VAR");
}

#[test]
fn resolve_api_keys_reads_from_env() {
    let toml = r#"
[profiles.test]
provider = "openai_compatible"
model_id = "test-model"
base_url = "https://api.example.com/v1"
api_key_env = "TEST_KEY_123"
"#;
    std::env::set_var("TEST_KEY_123", "sk-abc");
    let mut config = load_from_str(toml, "test.toml").unwrap();
    resolve_api_keys(&mut config);
    let (_, def) = &config.profiles[0];
    assert_eq!(def.api_key, Some("sk-abc".to_string()));
    std::env::remove_var("TEST_KEY_123");
}

#[test]
fn resolve_api_keys_does_not_overwrite_existing_key() {
    let toml = r#"
[profiles.test]
provider = "openai_compatible"
model_id = "test-model"
base_url = "https://api.example.com/v1"
api_key = "hardcoded"
api_key_env = "SOME_VAR"
"#;
    let mut config = load_from_str(toml, "test.toml").unwrap();
    resolve_api_keys(&mut config);
    let (_, def) = &config.profiles[0];
    assert_eq!(def.api_key, Some("hardcoded".to_string()));
}

#[test]
fn resolve_api_keys_noop_when_no_env_var() {
    let toml = r#"
[profiles.test]
provider = "openai_compatible"
model_id = "test-model"
base_url = "https://api.example.com/v1"
api_key_env = "NONEXISTENT_VAR_FOR_TEST"
"#;
    let mut config = load_from_str(toml, "test.toml").unwrap();
    resolve_api_keys(&mut config);
    let (_, def) = &config.profiles[0];
    assert_eq!(def.api_key, None);
}

#[test]
fn resolve_api_keys_fallback_empty_if_no_env_and_not_anthropic() {
    let toml = r#"
[profiles.test]
provider = "openai_compatible"
model_id = "test-model"
base_url = "https://api.example.com/v1"
api_key_env = "NONEXISTENT_VAR"
"#;
    let mut config = load_from_str(toml, "test.toml").unwrap();
    resolve_api_keys(&mut config);
    let (_, def) = &config.profiles[0];
    assert_eq!(def.api_key, None);
}

#[test]
fn headers_env_expansion() {
    std::env::set_var("TEST_MCP_TOKEN", "secret123");
    let input = "Bearer ${TEST_MCP_TOKEN}";
    let result = expand_env_vars(input);
    assert_eq!(result, "Bearer secret123");
    std::env::remove_var("TEST_MCP_TOKEN");
}

#[test]
fn headers_env_expansion_missing_var_keeps_placeholder() {
    std::env::remove_var("TEST_MCP_MISSING_VAR");
    let input = "Bearer ${TEST_MCP_MISSING_VAR}";
    let result = expand_env_vars(input);
    assert_eq!(result, "Bearer ${TEST_MCP_MISSING_VAR}");
}

#[test]
fn empty_env_value_resolves_from_env() {
    std::env::set_var("TEST_MCP_VAR", "resolved_value");
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.test]
type = "stdio"
command = "echo"

[mcp_servers.test.env]
TEST_MCP_VAR = ""
"#;
    let mut config = load_from_str(toml, "test.toml").unwrap();
    resolve_mcp_env(&mut config);
    let (_, server) = &config.mcp_servers[0];
    assert_eq!(
        server.env.as_ref().unwrap().get("TEST_MCP_VAR"),
        Some(&"resolved_value".to_string())
    );
    std::env::remove_var("TEST_MCP_VAR");
}

#[test]
fn non_empty_env_value_not_overwritten() {
    std::env::set_var("TEST_MCP_PRESERVED", "env_value");
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.test]
type = "stdio"
command = "echo"

[mcp_servers.test.env]
TEST_MCP_PRESERVED = "explicit_value"
"#;
    let mut config = load_from_str(toml, "test.toml").unwrap();
    resolve_mcp_env(&mut config);
    let (_, server) = &config.mcp_servers[0];
    assert_eq!(
        server.env.as_ref().unwrap().get("TEST_MCP_PRESERVED"),
        Some(&"explicit_value".to_string())
    );
    std::env::remove_var("TEST_MCP_PRESERVED");
}

#[test]
fn mcp_headers_with_env_expansion() {
    std::env::set_var("TEST_MCP_AUTH", "my-token-123");
    let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.test]
type = "sse"
url = "https://mcp.example.com"

[mcp_servers.test.headers]
Authorization = "Bearer ${TEST_MCP_AUTH}"
"#;
    let mut config = load_from_str(toml, "test.toml").unwrap();
    resolve_mcp_env(&mut config);
    let (_, server) = &config.mcp_servers[0];
    assert_eq!(
        server.headers.as_ref().unwrap().get("Authorization"),
        Some(&"Bearer my-token-123".to_string())
    );
    std::env::remove_var("TEST_MCP_AUTH");
}

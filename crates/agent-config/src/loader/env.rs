use crate::Config;

/// Resolve API keys: if `api_key_env` is set and `api_key` is not,
/// read the environment variable and populate `api_key`.
///
/// For Anthropic profiles, if the env var is not set, falls back to
/// reading `ANTHROPIC_AUTH_TOKEN` from `~/.claude/settings.json`.
pub fn resolve_api_keys(config: &mut Config) {
    for (alias, profile) in &mut config.profiles {
        if profile.api_key.is_none() {
            if let Some(ref env_var) = profile.api_key_env {
                if let Ok(key) = std::env::var(env_var) {
                    profile.api_key = Some(key);
                }
            }
        }

        // Fallback for Anthropic profiles: try ~/.claude/settings.json
        if profile.api_key.is_none() && profile.provider == "anthropic" {
            if let Some(key) = try_read_claude_auth_token() {
                eprintln!(
                    "[agent-config] profile '{}': resolved Anthropic API key from ~/.claude/settings.json (ANTHROPIC_AUTH_TOKEN)",
                    alias
                );
                profile.api_key = Some(key);
                // Also set the env var so that AnthropicConfig::api_key() can find it
                let env_name = format!("KAIROX_KEY_{}", alias.replace('-', "_").to_uppercase());
                std::env::set_var(&env_name, profile.api_key.as_ref().unwrap());
                profile.api_key_env = Some(env_name);
            }
        }
    }
}

/// Try to read `ANTHROPIC_AUTH_TOKEN` from `~/.claude/settings.json`.
fn try_read_claude_auth_token() -> Option<String> {
    let home = dirs::home_dir()?;
    let settings_path = home.join(".claude").join("settings.json");
    if !settings_path.is_file() {
        return None;
    }
    let content = std::fs::read_to_string(&settings_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    value
        .get("env")?
        .get("ANTHROPIC_AUTH_TOKEN")?
        .as_str()
        .map(|s| s.to_string())
}

/// Resolve environment variables in MCP server configs.
/// - env fields with empty values are resolved from env vars of the same name
/// - headers with `${VAR}` patterns are expanded from environment
pub fn resolve_mcp_env(config: &mut Config) {
    for (_id, server) in &mut config.mcp_servers {
        // Resolve empty env values
        if let Some(ref mut env) = server.env {
            for (key, value) in env.iter_mut() {
                if value.is_empty() {
                    if let Ok(resolved) = std::env::var(key) {
                        *value = resolved;
                    }
                }
            }
        }

        // Expand ${VAR} in headers
        if let Some(ref mut headers) = server.headers {
            for (_key, value) in headers.iter_mut() {
                *value = expand_env_vars(value);
            }
        }
    }
}

/// Expand `${VAR}` patterns in a string from environment variables.
fn expand_env_vars(input: &str) -> String {
    let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        std::env::var(&caps[1]).unwrap_or_else(|_| caps[0].to_string())
    })
    .to_string()
}

#[cfg(test)]
mod tests {
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
}

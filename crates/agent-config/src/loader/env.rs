use crate::Config;
use regex::Regex;
use std::sync::LazyLock;

static ENV_VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{([^}]+)\}").expect("env var regex must compile"));

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
    ENV_VAR_RE
        .replace_all(input, |caps: &regex::Captures| {
            std::env::var(&caps[1]).unwrap_or_else(|_| caps[0].to_string())
        })
        .to_string()
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;

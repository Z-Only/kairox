//! TOML parsing, API key resolution, and validation.

use crate::{Config, ConfigError, McpServerConfig, McpTransportType, ProfileDef};

/// Intermediate TOML structure for deserialization.
#[derive(Debug, serde::Deserialize)]
struct ConfigToml {
    #[serde(default)]
    profiles: toml::value::Table,
    #[serde(default)]
    mcp_servers: toml::value::Table,
    #[serde(default)]
    context: crate::ContextPolicy,
}

/// Intermediate profile structure for deserialization.
#[derive(Debug, serde::Deserialize)]
struct ProfileToml {
    provider: String,
    model_id: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    api_key_env: Option<String>,
    #[serde(default)]
    context_window: Option<u64>,
    #[serde(default)]
    output_limit: Option<u64>,
    #[serde(default)]
    response: Option<String>,
    // -- new fields --
    #[serde(default)]
    max_tokens: Option<u64>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    top_k: Option<u32>,
    #[serde(default)]
    headers: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    supports_tools: Option<bool>,
    #[serde(default)]
    supports_vision: Option<bool>,
    #[serde(default)]
    supports_reasoning: Option<bool>,
    #[serde(default)]
    extra_params: Option<toml::Value>,
}

/// Parse a TOML string into a Config.
pub fn load_from_str(content: &str, path_for_errors: &str) -> Result<Config, ConfigError> {
    let config_toml: ConfigToml = toml::from_str(content).map_err(|e| ConfigError::Parse {
        path: path_for_errors.to_string(),
        message: e.to_string(),
    })?;

    let mut profiles = Vec::new();

    for (alias, value) in &config_toml.profiles {
        let profile_toml: ProfileToml =
            value.clone().try_into().map_err(|e| ConfigError::Parse {
                path: path_for_errors.to_string(),
                message: format!("profile '{}': {}", alias, e),
            })?;

        let profile_def = ProfileDef {
            provider: profile_toml.provider,
            model_id: profile_toml.model_id,
            base_url: profile_toml.base_url,
            api_key: profile_toml.api_key,
            api_key_env: profile_toml.api_key_env,
            context_window: profile_toml.context_window,
            output_limit: profile_toml.output_limit,
            response: profile_toml.response,
            max_tokens: profile_toml.max_tokens,
            temperature: profile_toml.temperature,
            top_p: profile_toml.top_p,
            top_k: profile_toml.top_k,
            headers: profile_toml.headers,
            supports_tools: profile_toml.supports_tools,
            supports_vision: profile_toml.supports_vision,
            supports_reasoning: profile_toml.supports_reasoning,
            extra_params: profile_toml.extra_params,
        };

        profiles.push((alias.clone(), profile_def));
    }

    // Parse MCP server definitions
    let mut mcp_servers = Vec::new();
    for (id, value) in &config_toml.mcp_servers {
        let server_config: McpServerConfig =
            value.clone().try_into().map_err(|e| ConfigError::Parse {
                path: path_for_errors.to_string(),
                message: format!("mcp_server '{}': {}", id, e),
            })?;

        // Validate required fields per transport type
        match server_config.r#type {
            McpTransportType::Stdio if server_config.command.is_none() => {
                return Err(ConfigError::Parse {
                    path: path_for_errors.to_string(),
                    message: format!("mcp_server '{}': stdio requires 'command'", id),
                });
            }
            McpTransportType::Sse if server_config.url.is_none() => {
                return Err(ConfigError::Parse {
                    path: path_for_errors.to_string(),
                    message: format!("mcp_server '{}': sse requires 'url'", id),
                });
            }
            _ => {}
        }

        mcp_servers.push((id.clone(), server_config));
    }

    Ok(Config {
        profiles,
        mcp_servers,
        source: crate::ConfigSource::ProjectFile, // Will be overridden by caller
        context: config_toml.context,
    })
}

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
pub(crate) fn expand_env_vars(input: &str) -> String {
    let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        std::env::var(&caps[1]).unwrap_or_else(|_| caps[0].to_string())
    })
    .to_string()
}

/// Validate the configuration: check for missing required fields, etc.
pub fn validate(config: &Config) -> Result<(), ConfigError> {
    for (alias, profile) in &config.profiles {
        // openai_compatible requires base_url
        if profile.provider == "openai_compatible" && profile.base_url.is_none() {
            return Err(ConfigError::Parse {
                path: "config".to_string(),
                message: format!(
                    "profile '{}' uses 'openai_compatible' provider but missing 'base_url'",
                    alias
                ),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_context_policy_with_defaults_and_overrides() {
        // Defaults: omitting [context] yields the default ContextPolicy.
        let cfg_default: crate::Config = crate::loader::load_from_str(
            r#"
[profiles.fake]
provider = "fake"
model_id = "fake"
"#,
            "test.toml",
        )
        .unwrap();
        assert!(
            (cfg_default.context.auto_compact_threshold - 0.85_f32).abs() < 1e-6,
            "default threshold should be 0.85, got {}",
            cfg_default.context.auto_compact_threshold
        );
        assert!(cfg_default.context.compactor_profile.is_none());
        assert!(cfg_default.context.max_tool_definition_tokens.is_none());

        // Overrides: explicit values take precedence.
        let cfg_user: crate::Config = crate::loader::load_from_str(
            r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4o"
base_url = "https://api.openai.com/v1"

[context]
auto_compact_threshold = 0.7
compactor_profile = "fast"
max_tool_definition_tokens = 25000
"#,
            "test.toml",
        )
        .unwrap();
        assert!((cfg_user.context.auto_compact_threshold - 0.7).abs() < 1e-6);
        assert_eq!(cfg_user.context.compactor_profile.as_deref(), Some("fast"));
        assert_eq!(cfg_user.context.max_tool_definition_tokens, Some(25_000));
    }

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
        // omitted in TOML → None; `resolve_limits` will fall back to
        // builtin registry / provider default at usage time.
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
        // api_key was already set (direct), so it should remain unchanged
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
    fn parse_error_on_invalid_toml() {
        let toml = "this is not valid toml {{{{";
        let result = load_from_str(toml, "bad.toml");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::Parse { path, .. } => assert_eq!(path, "bad.toml"),
            _ => panic!("expected Parse error"),
        }
    }

    // -----------------------------------------------------------------------
    // MCP server parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_stdio_mcp_server() {
        let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.filesystem]
type = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
keep_alive = true
"#;
        let config = load_from_str(toml, "test.toml").unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        let (id, server) = &config.mcp_servers[0];
        assert_eq!(id, "filesystem");
        assert_eq!(server.r#type, McpTransportType::Stdio);
        assert_eq!(server.command, Some("npx".to_string()));
        assert!(server.keep_alive);
    }

    #[test]
    fn parse_sse_mcp_server() {
        let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.remote-search]
type = "sse"
url = "https://mcp.example.com/search"
api_key_env = "MCP_SEARCH_KEY"
"#;
        let config = load_from_str(toml, "test.toml").unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        let (id, server) = &config.mcp_servers[0];
        assert_eq!(id, "remote-search");
        assert_eq!(server.r#type, McpTransportType::Sse);
        assert_eq!(
            server.url,
            Some("https://mcp.example.com/search".to_string())
        );
    }

    #[test]
    fn reject_stdio_without_command() {
        let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.bad]
type = "stdio"
"#;
        let result = load_from_str(toml, "test.toml");
        assert!(result.is_err());
    }

    #[test]
    fn reject_sse_without_url() {
        let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.bad]
type = "sse"
"#;
        let result = load_from_str(toml, "test.toml");
        assert!(result.is_err());
    }

    #[test]
    fn mcp_default_values() {
        let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.test]
type = "stdio"
command = "echo"
"#;
        let config = load_from_str(toml, "test.toml").unwrap();
        let (_, server) = &config.mcp_servers[0];
        assert!(!server.keep_alive);
        assert_eq!(server.idle_timeout_secs, 300);
        assert!(server.auto_restart);
        assert_eq!(server.max_restart_attempts, 3);
    }

    #[test]
    fn mcp_server_defs_converts_to_agent_mcp_types() {
        let toml = r#"
[profiles.fake]
provider = "fake"
model_id = "fake"

[mcp_servers.fs]
type = "stdio"
command = "npx"
args = ["server-fs"]
"#;
        let config = load_from_str(toml, "test.toml").unwrap();
        let defs = config.mcp_server_defs();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "fs");
        assert!(matches!(
            defs[0].transport,
            agent_mcp::McpTransportDef::Stdio { .. }
        ));
        assert_eq!(defs[0].args, vec!["server-fs"]);
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
        // Ensure a missing env var leaves ${VAR} as-is
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

/// Load main config plus an optional marketplace `mcp_servers.toml` overlay.
///
/// Both sources contribute to `mcp_servers`. On id conflict, the main file
/// wins. Profiles, base config, etc. come solely from the main file.
pub fn load_with_marketplace_overlay(
    main_content: &str,
    marketplace_content: Option<&str>,
    main_path: &str,
    marketplace_path: &str,
) -> Result<Config, ConfigError> {
    let mut cfg = load_from_str(main_content, main_path)?;

    let Some(market) = marketplace_content else {
        return Ok(cfg);
    };

    let market_cfg = load_from_str(market, marketplace_path)?;
    let existing: std::collections::HashSet<String> =
        cfg.mcp_servers.iter().map(|(id, _)| id.clone()).collect();
    for (id, srv) in market_cfg.mcp_servers {
        if !existing.contains(&id) {
            cfg.mcp_servers.push((id, srv));
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod overlay_tests {
    use super::*;

    #[test]
    fn overlay_merges_marketplace_into_main_with_main_winning() {
        let main = r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4o-mini"
base_url = "https://api.openai.com/v1"

[mcp_servers.filesystem]
type = "stdio"
command = "main-fs"
args = []
"#;
        let market = r#"
[mcp_servers.filesystem]
type = "stdio"
command = "marketplace-fs"
args = []

[mcp_servers.brave-search]
type = "stdio"
command = "npx"
args = ["-y", "@mcp/brave"]
"#;
        let cfg = load_with_marketplace_overlay(main, Some(market), "kairox.toml", "mcp.toml")
            .expect("merge ok");
        let names: Vec<_> = cfg.mcp_servers.iter().map(|(id, _)| id.clone()).collect();
        assert!(names.contains(&"filesystem".to_string()));
        assert!(names.contains(&"brave-search".to_string()));
        let fs = cfg
            .mcp_servers
            .iter()
            .find(|(id, _)| id == "filesystem")
            .unwrap();
        assert_eq!(
            fs.1.command.as_deref(),
            Some("main-fs"),
            "main file wins on id conflict"
        );
    }

    #[test]
    fn overlay_with_no_marketplace_is_just_main() {
        let main = r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4o-mini"
base_url = "https://api.openai.com/v1"
"#;
        let cfg = load_with_marketplace_overlay(main, None, "k.toml", "m.toml").unwrap();
        assert!(cfg.mcp_servers.is_empty());
    }

    #[test]
    fn overlay_marketplace_only_servers_section_parses() {
        // Marketplace file has no [profiles.*] section — must still parse
        // because ConfigToml has #[serde(default)] on profiles.
        let main = r#"
[profiles.fast]
provider = "openai_compatible"
model_id = "gpt-4o-mini"
base_url = "https://api.openai.com/v1"
"#;
        let market = r#"
[mcp_servers.foo]
type = "stdio"
command = "foo"
args = []
"#;
        let cfg = load_with_marketplace_overlay(main, Some(market), "k.toml", "m.toml").unwrap();
        assert_eq!(cfg.mcp_servers.len(), 1);
        assert_eq!(cfg.mcp_servers[0].0, "foo");
    }
}

// ===========================================================================
// Phase 2: catalog source parsing
// ===========================================================================

/// Adapter kind for a remote catalog source. Mirrors
/// `agent_mcp::RemoteSourceKind` but lives here so `agent-config` does not
/// need to depend on `agent-mcp` (cycle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogSourceKind {
    McpRegistry,
}

/// A user-configured remote catalog source, parsed from `[[catalog_sources]]`
/// in the marketplace TOML. Mirrors `agent_mcp::RemoteSourceConfig`; the
/// runtime layer translates between the two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSourceConfig {
    pub id: String,
    pub display_name: String,
    pub kind: CatalogSourceKind,
    pub url: String,
    pub api_key_env: Option<String>,
    pub priority: u32,
    pub default_trust: String,
    pub enabled: bool,
    pub cache_ttl_seconds: Option<u64>,
}

/// Result bundle returned by [`load_with_marketplace_loaded`].
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: Config,
    pub catalog_sources: Vec<CatalogSourceConfig>,
}

#[derive(Debug, serde::Deserialize)]
struct MarketplaceTomlInner {
    #[serde(default)]
    #[allow(dead_code)]
    mcp_servers: toml::value::Table,
    #[serde(default)]
    catalog_sources: Vec<RawCatalogSource>,
}

#[derive(Debug, serde::Deserialize)]
struct RawCatalogSource {
    id: String,
    display_name: String,
    kind: String,
    url: String,
    #[serde(default)]
    api_key_env: Option<String>,
    #[serde(default = "default_priority")]
    priority: u32,
    #[serde(default = "default_trust_str")]
    default_trust: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    cache_ttl_seconds: Option<u64>,
}

fn default_priority() -> u32 {
    100
}
fn default_trust_str() -> String {
    "community".into()
}
fn default_true() -> bool {
    true
}

fn raw_to_source(raw: RawCatalogSource) -> Result<Option<CatalogSourceConfig>, ConfigError> {
    let kind = match raw.kind.as_str() {
        "mcp_registry" => CatalogSourceKind::McpRegistry,
        other => {
            eprintln!(
                "[agent-config] catalog_sources[{}]: skipping unsupported kind '{other}'",
                raw.id
            );
            return Ok(None);
        }
    };
    if !raw.url.starts_with("http://") && !raw.url.starts_with("https://") {
        return Err(ConfigError::Parse {
            path: "marketplace".into(),
            message: format!(
                "catalog_sources[{}]: url must start with http:// or https://",
                raw.id
            ),
        });
    }
    Ok(Some(CatalogSourceConfig {
        id: raw.id,
        display_name: raw.display_name,
        kind,
        url: raw.url,
        api_key_env: raw.api_key_env,
        priority: raw.priority,
        default_trust: raw.default_trust,
        enabled: raw.enabled,
        cache_ttl_seconds: raw.cache_ttl_seconds,
    }))
}

/// Parse only the `[[catalog_sources]]` array from a marketplace TOML
/// string. Returns an empty `Vec` if the section is missing. Entries
/// with unrecognised `kind` values are silently skipped so that old
/// config files written by a newer (or older) version of Kairox don't
/// prevent the application from starting.
pub fn parse_catalog_sources(
    marketplace_content: &str,
) -> Result<Vec<CatalogSourceConfig>, ConfigError> {
    let inner: MarketplaceTomlInner =
        toml::from_str(marketplace_content).map_err(|e| ConfigError::Parse {
            path: "marketplace".into(),
            message: e.to_string(),
        })?;
    inner
        .catalog_sources
        .into_iter()
        .filter_map(|raw| raw_to_source(raw).transpose())
        .collect()
}

/// Load main config + optional marketplace TOML, surfacing both MCP server
/// overlays (via [`load_with_marketplace_overlay`]) and Phase 2 catalog
/// sources.
pub fn load_with_marketplace_loaded(
    main_content: &str,
    marketplace_content: Option<&str>,
    main_path: &str,
    marketplace_path: &str,
) -> Result<LoadedConfig, ConfigError> {
    let config = load_with_marketplace_overlay(
        main_content,
        marketplace_content,
        main_path,
        marketplace_path,
    )?;
    let catalog_sources = match marketplace_content {
        Some(m) => parse_catalog_sources(m)?,
        None => vec![],
    };
    Ok(LoadedConfig {
        config,
        catalog_sources,
    })
}

#[cfg(test)]
mod catalog_sources_tests {
    use super::*;

    #[test]
    fn parses_catalog_sources_with_defaults() {
        let market = r#"
[[catalog_sources]]
id           = "mcp-registry"
display_name = "Model Context Protocol Servers"
kind         = "mcp_registry"
url          = "https://registry.modelcontextprotocol.io"
"#;
        let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
        assert_eq!(loaded.catalog_sources.len(), 1);
        let s = &loaded.catalog_sources[0];
        assert_eq!(s.id, "mcp-registry");
        assert_eq!(s.priority, 100);
        assert!(s.enabled);
        assert_eq!(s.default_trust, "community");
    }

    #[test]
    fn parses_multiple_sources_with_full_fields() {
        let market = r#"
[[catalog_sources]]
id            = "internal"
display_name  = "Internal"
kind          = "mcp_registry"
url           = "https://mcp.example.com/v0.1/servers"
api_key_env   = "INTERNAL_KEY"
priority      = 10
default_trust = "verified"
enabled       = true
cache_ttl_seconds = 600

[[catalog_sources]]
id           = "mcp-registry"
display_name = "Model Context Protocol Servers"
kind         = "mcp_registry"
url          = "https://registry.modelcontextprotocol.io"
priority     = 50
enabled      = false
"#;
        let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
        assert_eq!(loaded.catalog_sources.len(), 2);
        let internal = loaded
            .catalog_sources
            .iter()
            .find(|s| s.id == "internal")
            .unwrap();
        assert_eq!(internal.priority, 10);
        assert_eq!(internal.api_key_env.as_deref(), Some("INTERNAL_KEY"));
        assert_eq!(internal.cache_ttl_seconds, Some(600));
        let mcp = loaded
            .catalog_sources
            .iter()
            .find(|s| s.id == "mcp-registry")
            .unwrap();
        assert!(!mcp.enabled);
    }

    #[test]
    fn unknown_kind_is_silently_skipped() {
        let market = r#"
[[catalog_sources]]
id           = "x"
display_name = "X"
kind         = "wat"
url          = "https://x"
"#;
        let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
        // The source with unsupported kind is skipped, not an error.
        assert!(
            !loaded.catalog_sources.iter().any(|s| s.id == "x"),
            "unsupported kind should be silently dropped"
        );
    }

    #[test]
    fn rejects_invalid_url_scheme() {
        let market = r#"
[[catalog_sources]]
id           = "x"
display_name = "X"
kind         = "mcp_registry"
url          = "ftp://x"
"#;
        let err = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap_err();
        assert!(format!("{err:?}").to_lowercase().contains("url"));
    }

    #[test]
    fn missing_marketplace_yields_empty_sources() {
        let loaded = load_with_marketplace_loaded("", None, "k.toml", "m.toml").unwrap();
        assert!(loaded.catalog_sources.is_empty());
    }

    #[test]
    fn marketplace_with_only_mcp_servers_yields_empty_sources() {
        let market = r#"
[mcp_servers.foo]
type      = "stdio"
command   = "echo"
args      = []
"#;
        let loaded = load_with_marketplace_loaded("", Some(market), "k.toml", "m.toml").unwrap();
        assert_eq!(loaded.config.mcp_servers.len(), 1);
        assert!(loaded.catalog_sources.is_empty());
    }
}

// ===========================================================================
// Phase 2.1: built-in default remote catalog sources
// ===========================================================================

/// Default remote catalog sources shipped with Kairox so that the GUI
/// marketplace tab has visible subscriptions out of the box.
///
/// All defaults are `enabled = false`; users opt in by enabling them via
/// the GUI settings page (or by adding overriding entries to
/// `mcp_servers.toml`). User-defined sources with the same id replace the
/// matching default — see [`merge_with_defaults`].
pub fn default_catalog_sources() -> Vec<CatalogSourceConfig> {
    vec![CatalogSourceConfig {
        id: "mcp-registry".into(),
        display_name: "Model Context Protocol Servers".into(),
        kind: CatalogSourceKind::McpRegistry,
        url: "https://registry.modelcontextprotocol.io".into(),
        api_key_env: None,
        priority: 50,
        default_trust: "community".into(),
        enabled: false,
        cache_ttl_seconds: None,
    }]
}

/// Merge user-configured catalog sources with the built-in defaults.
///
/// Strategy: union by `id`. Any default whose `id` already appears in
/// `user_sources` is dropped (user overrides win), and remaining defaults
/// are appended after the user-provided entries. This preserves the
/// user's listing order while ensuring the predefined subscriptions are
/// always visible in the GUI even when no user config is present.
pub fn merge_with_defaults(user_sources: Vec<CatalogSourceConfig>) -> Vec<CatalogSourceConfig> {
    let user_ids: std::collections::HashSet<String> =
        user_sources.iter().map(|s| s.id.clone()).collect();
    let mut merged = user_sources;
    for default in default_catalog_sources() {
        if !user_ids.contains(&default.id) {
            merged.push(default);
        }
    }
    merged
}

#[cfg(test)]
mod default_catalog_sources_tests {
    use super::*;

    #[test]
    fn defaults_ship_disabled_remote_sources() {
        let defaults = default_catalog_sources();
        let ids: Vec<&str> = defaults.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"mcp-registry"));
        assert_eq!(defaults.len(), 1);
        assert!(
            defaults.iter().all(|s| !s.enabled),
            "defaults must ship disabled so GUI does not auto-fetch on cold start",
        );
        assert!(
            defaults
                .iter()
                .all(|s| s.url.starts_with("https://") || s.url.starts_with("http://")),
            "default urls must be well-formed http(s)",
        );
        let mcp = defaults.iter().find(|s| s.id == "mcp-registry").unwrap();
        assert_eq!(mcp.kind, CatalogSourceKind::McpRegistry);
        assert_eq!(mcp.url, "https://registry.modelcontextprotocol.io");
    }

    #[test]
    fn merge_with_defaults_seeds_all_when_user_empty() {
        let merged = merge_with_defaults(Vec::new());
        let ids: Vec<&str> = merged.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"mcp-registry"));
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_with_defaults_user_overrides_default_by_id() {
        let user = vec![CatalogSourceConfig {
            id: "mcp-registry".into(),
            display_name: "My MCP Mirror".into(),
            kind: CatalogSourceKind::McpRegistry,
            url: "https://my-mirror.example/v0.1/servers".into(),
            api_key_env: Some("MY_KEY".into()),
            priority: 10,
            default_trust: "verified".into(),
            enabled: true,
            cache_ttl_seconds: Some(120),
        }];
        let merged = merge_with_defaults(user);
        let mcp: Vec<_> = merged.iter().filter(|s| s.id == "mcp-registry").collect();
        assert_eq!(mcp.len(), 1);
        assert_eq!(mcp[0].display_name, "My MCP Mirror");
        assert!(mcp[0].enabled);
        assert_eq!(mcp[0].priority, 10);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_with_defaults_preserves_user_ordering_then_appends_defaults() {
        let user = vec![CatalogSourceConfig {
            id: "custom-first".into(),
            display_name: "Custom".into(),
            kind: CatalogSourceKind::McpRegistry,
            url: "https://custom.example/v0.1/servers".into(),
            api_key_env: None,
            priority: 5,
            default_trust: "community".into(),
            enabled: true,
            cache_ttl_seconds: None,
        }];
        let merged = merge_with_defaults(user);
        assert_eq!(merged[0].id, "custom-first");
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[1].id, "mcp-registry");
    }
}

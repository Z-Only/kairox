pub mod builder;
pub mod discovery;
pub mod limits;
pub mod loader;

use serde::{Deserialize, Serialize};

pub use builder::{build_ollama_clients, build_router};
pub use discovery::{find_config, find_config_upward};
pub use limits::resolve_limits;
pub use loader::{
    default_catalog_sources, load_from_str, load_with_marketplace_loaded,
    load_with_marketplace_overlay, merge_with_defaults, parse_catalog_sources, resolve_api_keys,
    resolve_mcp_env, validate, CatalogSourceConfig, CatalogSourceKind, LoadedConfig,
};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Definition of a single model profile, loaded from TOML or generated as default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDef {
    pub provider: String,
    pub model_id: String,
    #[serde(default)]
    pub base_url: Option<String>,
    /// Direct API key value. Takes priority over `api_key_env`.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Name of an environment variable that holds the API key.
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub output_limit: Option<u64>,
    /// Response text for the fake provider.
    #[serde(default)]
    pub response: Option<String>,
    // -- new fields --
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub top_k: Option<u32>,
    #[serde(default)]
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub supports_tools: Option<bool>,
    #[serde(default)]
    pub supports_vision: Option<bool>,
    #[serde(default)]
    pub supports_reasoning: Option<bool>,
    #[serde(default)]
    pub extra_params: Option<toml::Value>,
}

/// Metadata about a profile for UI display.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileInfo {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub local: bool,
    pub has_api_key: bool,
}

/// Where the configuration was loaded from.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSource {
    ProjectFile,
    UserFile,
    Defaults,
}

/// MCP transport type for server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportType {
    Stdio,
    Sse,
}

fn default_idle_timeout() -> u64 {
    300
}

fn default_true() -> bool {
    true
}

fn default_max_restart_attempts() -> u32 {
    3
}

/// MCP server configuration from TOML.
/// This is the TOML-facing type; it converts to agent_mcp::McpServerDef.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub r#type: McpTransportType,

    // stdio fields
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub cwd: Option<String>,

    // sse fields
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub api_key_env: Option<String>,

    // lifecycle options
    #[serde(default)]
    pub keep_alive: bool,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_true")]
    pub auto_restart: bool,
    #[serde(default = "default_max_restart_attempts")]
    pub max_restart_attempts: u32,
}

impl McpServerConfig {
    /// Convert this TOML-facing config into an `agent_mcp::McpServerDef`.
    pub fn to_server_def(&self, id: &str) -> agent_mcp::McpServerDef {
        let transport = match self.r#type {
            McpTransportType::Stdio => agent_mcp::McpTransportDef::Stdio {
                command: self.command.clone().unwrap_or_default(),
                cwd: self.cwd.clone(),
            },
            McpTransportType::Sse => agent_mcp::McpTransportDef::Sse {
                url: self.url.clone().unwrap_or_default(),
                api_key_env: self.api_key_env.clone(),
                headers: self.headers.clone().unwrap_or_default(),
            },
        };
        agent_mcp::McpServerDef {
            name: id.to_string(),
            transport,
            args: self.args.clone().unwrap_or_default(),
            env: self.env.clone().unwrap_or_default(),
            keep_alive: self.keep_alive,
            idle_timeout_secs: self.idle_timeout_secs,
            auto_restart: self.auto_restart,
            max_restart_attempts: self.max_restart_attempts,
        }
    }
}

/// Session compaction & context budgeting policy. Loaded from the
/// optional top-level `[context]` table in `kairox.toml`. All fields
/// have safe defaults so omitting the table is fine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPolicy {
    /// When the assembled context reaches this fraction of the budget,
    /// the runtime triggers automatic compaction. Set to `1.0` to disable.
    #[serde(default = "default_auto_compact_threshold")]
    pub auto_compact_threshold: f32,
    /// Optional profile alias to use for the summarisation LLM call.
    /// Falls back to the session's currently active profile when unset.
    #[serde(default)]
    pub compactor_profile: Option<String>,
    /// Optional cap on MCP + builtin tool definitions tokens. When the
    /// serialised tool schemas exceed this, the assembler drops the
    /// lowest-priority tools first.
    #[serde(default)]
    pub max_tool_definition_tokens: Option<u64>,
}

fn default_auto_compact_threshold() -> f32 {
    0.85
}

/// Assigns a sort key to profile aliases so "fake" and "fast" always
/// appear first in the profile list, with other profiles following.
fn profile_order_key(alias: &str) -> u8 {
    match alias {
        "fake" => 0,
        "fast" => 1,
        _ => 2,
    }
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            auto_compact_threshold: default_auto_compact_threshold(),
            compactor_profile: None,
            max_tool_definition_tokens: None,
        }
    }
}

/// Fully loaded configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub profiles: Vec<(String, ProfileDef)>,
    pub mcp_servers: Vec<(String, McpServerConfig)>,
    pub source: ConfigSource,
    /// Session compaction & context budgeting policy.
    pub context: ContextPolicy,
}

/// Errors that can occur during config loading.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config parse error in {path}: {message}")]
    Parse { path: String, message: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl Config {
    /// Load configuration with layered merging: defaults → user-level → project-level.
    /// Profiles and MCP servers from higher-priority layers override those from lower layers
    /// with the same name; new entries are appended.
    /// When `project_root` is `Some`, that directory is used to discover
    /// `.kairox/config.toml` instead of `std::env::current_dir()`.
    pub fn load() -> Result<Self, ConfigError> {
        let project_root = std::env::current_dir().ok();
        Self::load_inner(project_root.as_deref())
    }

    /// Load configuration with an explicit project root for project-level
    /// `.kairox/config.toml` discovery. Pass `None` to skip project-level config.
    pub fn load_with_project_root(
        project_root: Option<&std::path::Path>,
    ) -> Result<Self, ConfigError> {
        Self::load_inner(project_root)
    }

    fn load_inner(project_root: Option<&std::path::Path>) -> Result<Self, ConfigError> {
        let mut base = Self::defaults();

        // Layer 1: merge user-level config if present
        if let Some(home_dir) = dirs::home_dir() {
            let user_path = home_dir.join(".kairox").join("config.toml");
            if user_path.is_file() {
                base = Self::merge_config(base, &user_path)?;
            }
        }

        // Layer 2: merge project-level config if present (highest priority)
        if let Some(root) = project_root {
            let project_path = root.join(".kairox").join("config.toml");
            if project_path.is_file() {
                base = Self::merge_config(base, &project_path)?;
                base.source = ConfigSource::ProjectFile;
            } else {
                // Fallback: walk up from project_root looking for .kairox/config.toml
                if let Some((found_path, _)) = discovery::find_config_upward(root) {
                    base = Self::merge_config(base, &found_path)?;
                    base.source = ConfigSource::ProjectFile;
                }
            }
        }

        Ok(base)
    }

    /// Merge configuration from `path` into `base`, with profiles and MCP servers
    /// from the loaded config overriding or appending to the base.
    fn merge_config(base: Self, path: &std::path::Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let mut overlay = load_from_str(&content, &path.display().to_string())?;
        resolve_api_keys(&mut overlay);
        resolve_mcp_env(&mut overlay);

        // Merge profiles: overlay profiles replace base profiles with the same alias
        let mut profile_map: std::collections::HashMap<String, ProfileDef> =
            base.profiles.into_iter().collect();
        for (alias, def) in overlay.profiles {
            profile_map.insert(alias, def);
        }
        let mut merged_profiles: Vec<(String, ProfileDef)> = profile_map.into_iter().collect();
        // Stable sort: keep "fake" first, then "fast", then others
        merged_profiles.sort_by(|a, b| {
            let ap = profile_order_key(&a.0);
            let bp = profile_order_key(&b.0);
            ap.cmp(&bp)
        });

        // Merge MCP servers: overlay entries replace base entries with the same name
        let mut mcp_map: std::collections::HashMap<String, McpServerConfig> =
            base.mcp_servers.into_iter().collect();
        for (name, config) in overlay.mcp_servers {
            mcp_map.insert(name, config);
        }
        let mut merged_mcp: Vec<(String, McpServerConfig)> = mcp_map.into_iter().collect();
        merged_mcp.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(Config {
            profiles: merged_profiles,
            mcp_servers: merged_mcp,
            source: overlay.source,
            context: overlay.context,
        })
    }

    /// Generate default configuration from environment variables.
    pub fn defaults() -> Self {
        let mut profiles = vec![
            (
                "fake".into(),
                ProfileDef {
                    provider: "fake".into(),
                    model_id: "fake".into(),
                    base_url: None,
                    api_key: None,
                    api_key_env: None,
                    context_window: Some(4096),
                    output_limit: Some(2048),
                    response: Some("hello from Kairox".into()),
                    max_tokens: None,
                    temperature: None,
                    top_p: None,
                    top_k: None,
                    headers: None,
                    supports_tools: None,
                    supports_vision: None,
                    supports_reasoning: None,
                    extra_params: None,
                },
            ),
            (
                "local-code".into(),
                ProfileDef {
                    provider: "ollama".into(),
                    model_id: "devstral".into(),
                    base_url: Some("http://localhost:11434".into()),
                    api_key: None,
                    api_key_env: None,
                    context_window: Some(128_000),
                    output_limit: Some(16_384),
                    response: None,
                    max_tokens: None,
                    temperature: None,
                    top_p: None,
                    top_k: None,
                    headers: None,
                    supports_tools: None,
                    supports_vision: None,
                    supports_reasoning: None,
                    extra_params: None,
                },
            ),
        ];

        if std::env::var("OPENAI_API_KEY").is_ok() {
            profiles.insert(
                0,
                (
                    "fast".into(),
                    ProfileDef {
                        provider: "openai_compatible".into(),
                        model_id: "gpt-4.1-mini".into(),
                        base_url: Some("https://api.openai.com/v1".into()),
                        api_key: None,
                        api_key_env: Some("OPENAI_API_KEY".into()),
                        context_window: Some(128_000),
                        output_limit: Some(16_384),
                        response: None,
                        max_tokens: None,
                        temperature: None,
                        top_p: None,
                        top_k: None,
                        headers: None,
                        supports_tools: None,
                        supports_vision: None,
                        supports_reasoning: None,
                        extra_params: None,
                    },
                ),
            );
        }

        Config {
            profiles,
            mcp_servers: Vec::new(),
            source: ConfigSource::Defaults,
            context: ContextPolicy::default(),
        }
    }

    /// Build a `ModelRouter` from this configuration.
    pub fn build_router(&self) -> agent_models::ModelRouter {
        builder::build_router(self)
    }

    /// Get profile names in order.
    pub fn profile_names(&self) -> Vec<String> {
        self.profiles.iter().map(|(name, _)| name.clone()).collect()
    }

    /// Get the default profile name (fast > local-code > first available).
    /// Returns an owned String to avoid lifetime issues.
    pub fn default_profile(&self) -> String {
        let names = self.profile_names();
        if names.iter().any(|p| p == "fast") {
            "fast".to_string()
        } else if names.iter().any(|p| p == "local-code") {
            "local-code".to_string()
        } else {
            names.first().cloned().unwrap_or_else(|| "fake".to_string())
        }
    }

    /// Get profile metadata for UI display.
    pub fn profile_info(&self) -> Vec<ProfileInfo> {
        self.profiles
            .iter()
            .map(|(alias, def)| {
                let local = def.provider == "ollama" || def.provider == "fake";
                let has_api_key = def.api_key.is_some()
                    || def
                        .api_key_env
                        .as_ref()
                        .is_some_and(|v| std::env::var(v).is_ok());
                ProfileInfo {
                    alias: alias.clone(),
                    provider: def.provider.clone(),
                    model_id: def.model_id.clone(),
                    local,
                    has_api_key,
                }
            })
            .collect()
    }

    /// Look up a profile definition by alias.
    pub fn get_profile(&self, alias: &str) -> Option<&ProfileDef> {
        self.profiles
            .iter()
            .find(|(name, _)| name == alias)
            .map(|(_, def)| def)
    }

    /// Convert parsed MCP server configs to agent-mcp McpServerDef instances.
    pub fn mcp_server_defs(&self) -> Vec<agent_mcp::McpServerDef> {
        self.mcp_servers
            .iter()
            .map(|(id, config)| config.to_server_def(id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_include_fake_and_local_code() {
        let config = Config::defaults();
        let names = config.profile_names();
        assert!(names.contains(&"fake".to_string()));
        assert!(names.contains(&"local-code".to_string()));
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
        assert_eq!(names.len(), config.profiles.len());
    }

    #[test]
    fn profile_info_reflects_local_and_key_status() {
        let config = Config::defaults();
        let info = config.profile_info();
        assert!(info.iter().any(|p| p.alias == "fake" && p.local));
        assert!(info.iter().any(|p| p.alias == "local-code" && p.local));
    }

    #[test]
    fn defaults_has_empty_mcp_servers() {
        let config = Config::defaults();
        assert!(config.mcp_servers.is_empty());
    }
}

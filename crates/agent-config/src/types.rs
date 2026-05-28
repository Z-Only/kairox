use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub supports_tools: Option<bool>,
    #[serde(default)]
    pub supports_vision: Option<bool>,
    #[serde(default)]
    pub supports_reasoning: Option<bool>,
    #[serde(default)]
    pub extra_params: Option<toml::Value>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Resolve whether a profile exposes user-selectable reasoning effort.
///
/// `supports_reasoning` remains an explicit override, but known reasoning
/// models should work out of the box so GUI model switching can surface the
/// effort picker for profiles that users configure manually.
pub fn profile_supports_reasoning(def: &ProfileDef) -> bool {
    def.supports_reasoning
        .unwrap_or_else(|| model_supports_reasoning(&def.provider, &def.model_id))
}

fn model_supports_reasoning(provider: &str, model_id: &str) -> bool {
    let provider = provider.to_ascii_lowercase();
    let model_id = model_id.to_ascii_lowercase();
    let is_claude = provider == "anthropic" || model_id.contains("claude");

    is_claude
        && (model_id.contains("claude-opus-4")
            || model_id.contains("claude-sonnet-4")
            || model_id.contains("claude-3-7-sonnet"))
}

/// Metadata about a profile for UI display.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileInfo {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub local: bool,
    pub has_api_key: bool,
    pub supports_reasoning: bool,
    #[serde(default)]
    pub provider_display: String,
    #[serde(default)]
    pub model_display: String,
}

/// Where the configuration was loaded from.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSource {
    ProjectFile,
    UserFile,
    LocalFile,
    Defaults,
}

/// MCP transport type for server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportType {
    Stdio,
    Sse,
    StreamableHttp,
}

pub(crate) fn default_idle_timeout() -> u64 {
    300
}

pub(crate) fn default_true() -> bool {
    true
}

pub(crate) fn default_max_restart_attempts() -> u32 {
    3
}

pub(crate) fn default_hooks_enabled() -> bool {
    true
}

/// Feature flags loaded from the optional top-level `[features]` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeatureFlags {
    #[serde(default = "default_hooks_enabled")]
    pub hooks: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            hooks: default_hooks_enabled(),
        }
    }
}

/// Supported hook lifecycle events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PermissionRequest,
    PostToolUse,
    Stop,
}

impl HookEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            HookEvent::SessionStart => "SessionStart",
            HookEvent::UserPromptSubmit => "UserPromptSubmit",
            HookEvent::PreToolUse => "PreToolUse",
            HookEvent::PermissionRequest => "PermissionRequest",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::Stop => "Stop",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "SessionStart" => Some(Self::SessionStart),
            "UserPromptSubmit" => Some(Self::UserPromptSubmit),
            "PreToolUse" => Some(Self::PreToolUse),
            "PermissionRequest" => Some(Self::PermissionRequest),
            "PostToolUse" => Some(Self::PostToolUse),
            "Stop" => Some(Self::Stop),
            _ => None,
        }
    }
}

impl std::fmt::Display for HookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Command hook loaded from `[hooks.<event>.<id>]` in `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookConfig {
    pub id: String,
    pub event: HookEvent,
    #[serde(default)]
    pub matcher: Option<String>,
    pub command: String,
    #[serde(default)]
    pub status_message: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct HookConfigToml {
    #[serde(default)]
    pub matcher: Option<String>,
    pub command: String,
    #[serde(default)]
    pub status_message: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl HookConfigToml {
    pub(crate) fn into_hook_config(self, event: HookEvent, id: String) -> HookConfig {
        HookConfig {
            id,
            event,
            matcher: self.matcher,
            command: self.command,
            status_message: self.status_message,
            timeout_secs: self.timeout_secs,
            enabled: self.enabled,
        }
    }
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
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub cwd: Option<String>,

    // sse fields
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
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
            McpTransportType::StreamableHttp => agent_mcp::McpTransportDef::StreamableHttp {
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

pub(crate) fn default_auto_compact_threshold() -> f32 {
    0.85
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
    /// MCP server IDs disabled at the project (or higher) scope.
    /// Each entry is marked `disabled_by = Some(ConfigScope::Project)` in the
    /// effective view so the project can override user-level servers.
    pub disabled_mcp_servers: Vec<String>,
    /// Merged custom instructions appended after the system prompt.
    /// Higher layers are concatenated with `\n\n` separator.
    pub instructions: Option<String>,
    /// Feature flags from `[features]`.
    pub features: FeatureFlags,
    /// Command hooks loaded from `[hooks.<event>.<id>]` tables.
    pub hooks: Vec<HookConfig>,
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
    /// Load configuration with layered merging: defaults -> user-level -> project-level.
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

        // Layer 1: User config (~/.kairox/config.toml)
        if let Some(home_dir) = dirs::home_dir() {
            let user_path = home_dir.join(".kairox").join("config.toml");
            if user_path.is_file() {
                base = Self::merge_config(base, &user_path, ConfigSource::UserFile)?;
            }
        }

        // Layer 2: Project config (.kairox/config.toml)
        if let Some(root) = project_root {
            let project_path = root.join(".kairox").join("config.toml");
            if project_path.is_file() {
                base = Self::merge_config(base, &project_path, ConfigSource::ProjectFile)?;
            } else {
                // Fallback: walk up from project_root looking for .kairox/config.toml
                if let Some((found_path, _)) = crate::discovery::find_config_upward(root) {
                    base = Self::merge_config(base, &found_path, ConfigSource::ProjectFile)?;
                }
            }
        }

        // Layer 3: Local config (.kairox/config.local.toml, gitignored)
        if let Some(local_path) = crate::discovery::find_local_config(project_root) {
            base = Self::merge_config(base, &local_path, ConfigSource::LocalFile)?;
        }

        Ok(base)
    }

    /// Merge configuration from `path` into `base`, with profiles and MCP servers
    /// from the loaded config overriding or appending to the base.
    fn merge_config(
        base: Self,
        path: &std::path::Path,
        source: ConfigSource,
    ) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let mut overlay = crate::loader::load_from_str(&content, &path.display().to_string())?;
        crate::loader::resolve_api_keys(&mut overlay);
        crate::loader::resolve_mcp_env(&mut overlay);

        // Merge profiles: overlay profiles replace base profiles with the same alias
        let mut profile_map: HashMap<String, ProfileDef> = base.profiles.into_iter().collect();
        for (alias, def) in overlay.profiles {
            profile_map.insert(alias, def);
        }
        let merged_profiles: Vec<(String, ProfileDef)> = profile_map.into_iter().collect();

        // Merge MCP servers: overlay entries replace base entries with the same name
        let mut mcp_map: HashMap<String, McpServerConfig> = base.mcp_servers.into_iter().collect();
        for (name, config) in overlay.mcp_servers {
            mcp_map.insert(name, config);
        }
        let mut merged_mcp: Vec<(String, McpServerConfig)> = mcp_map.into_iter().collect();
        merged_mcp.sort_by(|a, b| a.0.cmp(&b.0));

        // Merge disabled MCP server IDs: additive union across all layers.
        let mut disabled_set: std::collections::HashSet<String> =
            base.disabled_mcp_servers.into_iter().collect();
        disabled_set.extend(overlay.disabled_mcp_servers);
        let merged_disabled: Vec<String> = disabled_set.into_iter().collect();

        // Merge instructions: higher layer appends to lower layer with "\n\n".
        let merged_instructions = match (&base.instructions, &overlay.instructions) {
            (Some(b), Some(o)) if !b.is_empty() && !o.is_empty() => Some(format!("{}\n\n{}", b, o)),
            (Some(b), _) if !b.is_empty() => Some(b.clone()),
            (_, Some(o)) if !o.is_empty() => Some(o.clone()),
            _ => None,
        };

        let mut hooks_map: HashMap<(HookEvent, String), HookConfig> = base
            .hooks
            .into_iter()
            .map(|hook| ((hook.event, hook.id.clone()), hook))
            .collect();
        for hook in overlay.hooks {
            hooks_map.insert((hook.event, hook.id.clone()), hook);
        }
        let mut merged_hooks: Vec<HookConfig> = hooks_map.into_values().collect();
        merged_hooks.sort_by(|left, right| {
            left.event
                .as_str()
                .cmp(right.event.as_str())
                .then_with(|| left.id.cmp(&right.id))
        });

        Ok(Config {
            profiles: merged_profiles,
            mcp_servers: merged_mcp,
            source,
            context: overlay.context,
            disabled_mcp_servers: merged_disabled,
            instructions: merged_instructions,
            features: overlay.features,
            hooks: merged_hooks,
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
                    enabled: true,
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
                    enabled: false,
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
                        enabled: true,
                    },
                ),
            );
        }

        Config {
            profiles,
            mcp_servers: Vec::new(),
            source: ConfigSource::Defaults,
            context: ContextPolicy::default(),
            disabled_mcp_servers: Vec::new(),
            instructions: None,
            features: FeatureFlags::default(),
            hooks: Vec::new(),
        }
    }

    /// Build a `ModelRouter` from this configuration.
    pub fn build_router(&self) -> agent_models::ModelRouter {
        crate::builder::build_router(self)
    }

    /// Get profile names in order.
    pub fn profile_names(&self) -> Vec<String> {
        self.profiles
            .iter()
            .filter(|(_, def)| def.enabled)
            .map(|(name, _)| name.clone())
            .collect()
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
            .filter(|(_, def)| def.enabled)
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
                    supports_reasoning: profile_supports_reasoning(def),
                    provider_display: def.provider.clone(),
                    model_display: def.model_id.clone(),
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

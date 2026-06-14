use std::collections::HashMap;

use super::context::{AdvisorConfig, ContextPolicy, FeatureFlags};
use super::hooks::{HookConfig, HookEvent};
use super::knowledge_base::KnowledgeBaseConfig;
use super::lsp::{DapServerConfig, LspServerConfig};
use super::mcp::McpServerConfig;
use super::profile::{profile_supports_reasoning, ConfigSource, ProfileDef, ProfileInfo};

/// Fully loaded configuration.
#[derive(Debug, Clone)]
pub struct Config {
    pub profiles: Vec<(String, ProfileDef)>,
    pub mcp_servers: Vec<(String, McpServerConfig)>,
    pub knowledge_bases: Vec<(String, KnowledgeBaseConfig)>,
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
    /// LSP server configurations from `[lsp_servers.<id>]` tables.
    pub lsp_servers: Vec<(String, LspServerConfig)>,
    /// DAP server configurations from `[dap_servers.<id>]` tables.
    pub dap_servers: Vec<(String, DapServerConfig)>,
    /// Advisor (self-reflection) configuration from `[advisor]`.
    pub advisor: AdvisorConfig,
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
        // without changing that alias' position; new profiles append in overlay order.
        let mut merged_profiles = base.profiles;
        for (alias, def) in overlay.profiles {
            if let Some((_, existing_def)) =
                merged_profiles.iter_mut().find(|(name, _)| name == &alias)
            {
                *existing_def = def;
            } else {
                merged_profiles.push((alias, def));
            }
        }

        // Merge MCP servers: overlay entries replace base entries with the same name
        let mut mcp_map: HashMap<String, McpServerConfig> = base.mcp_servers.into_iter().collect();
        for (name, config) in overlay.mcp_servers {
            mcp_map.insert(name, config);
        }
        let mut merged_mcp: Vec<(String, McpServerConfig)> = mcp_map.into_iter().collect();
        merged_mcp.sort_by(|a, b| a.0.cmp(&b.0));

        // Merge knowledge bases: overlay entries replace base entries with the same ID.
        let mut kb_map: HashMap<String, KnowledgeBaseConfig> =
            base.knowledge_bases.into_iter().collect();
        for (id, config) in overlay.knowledge_bases {
            kb_map.insert(id, config);
        }
        let mut merged_kb: Vec<(String, KnowledgeBaseConfig)> = kb_map.into_iter().collect();
        merged_kb.sort_by(|a, b| a.0.cmp(&b.0));

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

        // Merge LSP servers
        let mut lsp_map: HashMap<String, LspServerConfig> = base.lsp_servers.into_iter().collect();
        for (name, config) in overlay.lsp_servers {
            lsp_map.insert(name, config);
        }
        let mut merged_lsp: Vec<(String, LspServerConfig)> = lsp_map.into_iter().collect();
        merged_lsp.sort_by(|a, b| a.0.cmp(&b.0));

        // Merge DAP servers
        let mut dap_map: HashMap<String, DapServerConfig> = base.dap_servers.into_iter().collect();
        for (name, config) in overlay.dap_servers {
            dap_map.insert(name, config);
        }
        let mut merged_dap: Vec<(String, DapServerConfig)> = dap_map.into_iter().collect();
        merged_dap.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(Config {
            profiles: merged_profiles,
            mcp_servers: merged_mcp,
            knowledge_bases: merged_kb,
            source,
            context: overlay.context,
            disabled_mcp_servers: merged_disabled,
            instructions: merged_instructions,
            features: overlay.features,
            hooks: merged_hooks,
            lsp_servers: merged_lsp,
            dap_servers: merged_dap,
            advisor: if overlay.advisor.is_default() {
                base.advisor
            } else {
                overlay.advisor
            },
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
                    client_identity: None,
                    supports_tools: None,
                    supports_vision: None,
                    supports_reasoning: None,
                    extra_params: None,
                    server_tool_code_execution: None,
                    server_tool_web_search: None,
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
                    client_identity: None,
                    supports_tools: None,
                    supports_vision: None,
                    supports_reasoning: None,
                    extra_params: None,
                    server_tool_code_execution: None,
                    server_tool_web_search: None,
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
                        client_identity: None,
                        supports_tools: None,
                        supports_vision: None,
                        supports_reasoning: None,
                        extra_params: None,
                        server_tool_code_execution: None,
                        server_tool_web_search: None,
                        enabled: true,
                    },
                ),
            );
        }

        Config {
            profiles,
            mcp_servers: Vec::new(),
            knowledge_bases: Vec::new(),
            source: ConfigSource::Defaults,
            context: ContextPolicy::default(),
            disabled_mcp_servers: Vec::new(),
            instructions: None,
            features: FeatureFlags::default(),
            hooks: Vec::new(),
            lsp_servers: Vec::new(),
            dap_servers: Vec::new(),
            advisor: AdvisorConfig::default(),
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
                    context_window: def.context_window,
                    supports_vision: def.supports_vision.unwrap_or(false),
                    supports_tools: def.supports_tools.unwrap_or(false),
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

    /// Convert parsed LSP server configs to agent-lsp LspServerDef instances.
    pub fn lsp_server_defs(&self) -> Vec<agent_lsp::LspServerDef> {
        self.lsp_servers
            .iter()
            .map(|(id, config)| config.to_server_def(id))
            .collect()
    }

    /// Convert parsed DAP server configs to agent-lsp DapServerDef instances.
    pub fn dap_server_defs(&self) -> Vec<agent_lsp::DapServerDef> {
        self.dap_servers
            .iter()
            .map(|(id, config)| config.to_server_def(id))
            .collect()
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;

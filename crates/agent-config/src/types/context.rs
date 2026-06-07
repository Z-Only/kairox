use serde::{Deserialize, Serialize};

pub(crate) fn default_hooks_enabled() -> bool {
    true
}

/// Advisor (self-reflection) configuration. Loaded from the optional
/// `[advisor]` table in `kairox.toml`.
///
/// Example:
/// ```toml
/// [advisor]
/// mode = "lightweight"           # "off" | "lightweight" | "full"
/// profile = "haiku"              # model profile alias for the advisor
/// max_concerns = 5               # cap on reported concerns per review
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvisorConfig {
    /// How aggressively the advisor reviews tool calls.
    #[serde(default)]
    pub mode: agent_core::AdvisorMode,
    /// Model profile alias to use for advisor reviews. Falls back to the
    /// session's currently active profile when unset. Use a cheaper/faster
    /// model (e.g. Haiku) to keep latency and cost low.
    #[serde(default)]
    pub profile: Option<String>,
    /// Maximum number of concerns the advisor should report per review.
    #[serde(default = "default_max_concerns")]
    pub max_concerns: usize,
}

fn default_max_concerns() -> usize {
    5
}

impl Default for AdvisorConfig {
    fn default() -> Self {
        Self {
            mode: agent_core::AdvisorMode::Off,
            profile: None,
            max_concerns: default_max_concerns(),
        }
    }
}

impl AdvisorConfig {
    /// Returns `true` when every field matches the default values, meaning the
    /// `[advisor]` table was likely omitted rather than explicitly set.
    pub fn is_default(&self) -> bool {
        self.mode == agent_core::AdvisorMode::Off
            && self.profile.is_none()
            && self.max_concerns == default_max_concerns()
    }
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

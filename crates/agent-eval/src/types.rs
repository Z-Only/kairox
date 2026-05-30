//! Data, config, and result types for the evaluation harness.
//!
//! These are the plain types consumed and produced by [`crate::EvalHarness`]
//! and the harness IO/eval free functions in [`crate`]. They are re-exported
//! from the crate root, so external paths such as `agent_eval::EvalScenario`
//! remain unchanged.

use agent_config::Config;
use agent_core::DomainEvent;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, EvalError>;

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("scenario parse error on line {line}: {source}")]
    ScenarioParse {
        line: usize,
        source: serde_json::Error,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("config error: {0}")]
    Config(#[from] agent_config::ConfigError),
    #[error("store error: {0}")]
    Store(#[from] agent_store::StoreError),
    #[error("memory error: {0}")]
    Memory(#[from] agent_memory::MemoryStoreError),
    #[error("runtime error: {0}")]
    Runtime(#[from] agent_core::CoreError),
    #[error("invalid policy: {0}")]
    Policy(String),
    #[error("{0}")]
    Cli(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvalScenario {
    pub id: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<ApprovalPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<SandboxPolicy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub expected: EvalExpectation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvalExpectation {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assistant_contains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub event_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden_event_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_tool_invocations: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_failures: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct EvalRunOptions {
    pub workspace_root: PathBuf,
    pub default_profile: Option<String>,
    pub config: Option<Config>,
    pub approval_policy: ApprovalPolicy,
    pub sandbox_policy: SandboxPolicy,
    pub include_trace: bool,
    pub enable_mcp: bool,
    pub enable_hooks: bool,
    /// Override `config.context.auto_compact_threshold` for this run. Use
    /// a small value (for example `0.001`) on the `fake` profile to force
    /// a single-turn auto-compaction; leave `None` to keep the
    /// project/default value (typically `0.85`).
    pub auto_compact_threshold: Option<f32>,
    /// When true, re-register the `fake` profile on the router with a
    /// `FakeModelClient` that emits a tool-call after its token stream.
    /// Combine with `fake_tool_id` / `fake_tool_arguments` to control the
    /// tool invoked. Defaults to `fs.list {"path":"."}` which is safe in
    /// any temp workspace.
    pub fake_emit_tool_call: bool,
    pub fake_tool_id: Option<String>,
    pub fake_tool_arguments: Option<serde_json::Value>,
    /// When set, after `send_message` returns, [`crate::EvalHarness::run_scenario`]
    /// polls the persisted trace until every event listed in
    /// `scenario.expected.event_types` is present or this many milliseconds
    /// have elapsed. Needed for events emitted by detached background tasks
    /// such as auto-compaction.
    pub wait_timeout_ms: Option<u64>,
    /// Seed this many synthetic `UserMessageAdded`/`AssistantMessageCompleted`
    /// pairs directly into the event store before each `send_message`.
    /// `compaction::pick_compaction_boundary` needs at least
    /// `KEEP_LAST_PAIRS + 1` complete pairs (currently 4) to emit
    /// `ContextCompactionStarted`/`Completed`; a single fake turn alone is
    /// silently dropped. Use `4` to drive a deterministic compaction smoke.
    pub seed_synthetic_pairs: Option<usize>,
}

impl Default for EvalRunOptions {
    fn default() -> Self {
        Self {
            workspace_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            default_profile: None,
            config: None,
            approval_policy: ApprovalPolicy::OnRequest,
            sandbox_policy: SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
            include_trace: false,
            enable_mcp: false,
            enable_hooks: false,
            auto_compact_threshold: None,
            fake_emit_tool_call: false,
            fake_tool_id: None,
            fake_tool_arguments: None,
            wait_timeout_ms: None,
            seed_synthetic_pairs: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EvalResult {
    pub scenario_id: String,
    pub profile: String,
    pub passed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_response: Option<String>,
    #[serde(default)]
    pub event_types: Vec<String>,
    #[serde(default)]
    pub tool_invocations: usize,
    #[serde(default)]
    pub tool_failures: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<Vec<DomainEvent>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub success_rate: f64,
    pub total_elapsed_ms: u64,
    pub avg_elapsed_ms: f64,
    pub total_tool_invocations: usize,
    pub total_tool_failures: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_context_input_tokens: Option<u64>,
}

impl EvalSummary {
    pub fn from_results(results: &[EvalResult]) -> Self {
        let total = results.len();
        let passed = results.iter().filter(|result| result.passed).count();
        let failed = total.saturating_sub(passed);
        let total_elapsed_ms = results.iter().map(|result| result.elapsed_ms).sum();
        let total_tool_invocations = results.iter().map(|result| result.tool_invocations).sum();
        let total_tool_failures = results.iter().map(|result| result.tool_failures).sum();
        let token_values: Vec<u64> = results
            .iter()
            .filter_map(|result| result.context_input_tokens)
            .collect();
        let total_context_input_tokens = if token_values.is_empty() {
            None
        } else {
            Some(token_values.into_iter().sum())
        };

        Self {
            total,
            passed,
            failed,
            success_rate: if total == 0 {
                0.0
            } else {
                passed as f64 / total as f64
            },
            total_elapsed_ms,
            avg_elapsed_ms: if total == 0 {
                0.0
            } else {
                total_elapsed_ms as f64 / total as f64
            },
            total_tool_invocations,
            total_tool_failures,
            total_context_input_tokens,
        }
    }
}

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
use std::collections::HashMap;
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
    #[error("invalid regex `{pattern}`: {source}")]
    Regex {
        pattern: String,
        source: regex::Error,
    },
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turns: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_instructions: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvalExpectation {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assistant_contains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assistant_not_contains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assistant_matches_regex: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub event_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden_event_types: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub min_events_of_type: HashMap<String, usize>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub max_events_of_type: HashMap<String, usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_tool_invocations: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_failures: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_elapsed_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_context_input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trajectory_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_trajectory_steps: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspace_files: Vec<EvalFileExpectation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post_run_commands: Vec<EvalCommandExpectation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvalFileExpectation {
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_contains: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalCommandExpectation {
    pub program: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(
        default = "default_command_exit_code",
        skip_serializing_if = "is_default_command_exit_code"
    )]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stdout_contains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stderr_contains: Vec<String>,
}

impl Default for EvalCommandExpectation {
    fn default() -> Self {
        Self {
            program: String::new(),
            args: Vec::new(),
            cwd: None,
            exit_code: default_command_exit_code(),
            timeout_ms: None,
            stdout_contains: Vec::new(),
            stderr_contains: Vec::new(),
        }
    }
}

fn default_command_exit_code() -> Option<i32> {
    Some(0)
}

fn is_default_command_exit_code(exit_code: &Option<i32>) -> bool {
    *exit_code == default_command_exit_code()
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
    /// Override `config.context.auto_compact_threshold` for this run.
    pub auto_compact_threshold: Option<f32>,
    /// When true, re-register the `fake` profile on the router with a
    /// `FakeModelClient` that emits a tool-call after its token stream.
    pub fake_emit_tool_call: bool,
    pub fake_tool_id: Option<String>,
    pub fake_tool_arguments: Option<serde_json::Value>,
    /// When set, polls the persisted trace until every event listed in
    /// `scenario.expected.event_types` is present or timeout elapses.
    pub wait_timeout_ms: Option<u64>,
    /// Hard wall-clock timeout for each scenario in batch runs.
    pub scenario_timeout_ms: Option<u64>,
    /// Seed synthetic history pairs before each `send_message`.
    pub seed_synthetic_pairs: Option<usize>,
    /// Permit scenario-controlled post-run commands after model execution.
    ///
    /// This is off by default because scenario JSONL is user-supplied input;
    /// live eval fixtures opt in explicitly when they need independent build
    /// or test verification of generated code.
    pub allow_post_run_commands: bool,
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
            scenario_timeout_ms: None,
            seed_synthetic_pairs: None,
            allow_post_run_commands: false,
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
    #[serde(default)]
    pub turns_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trajectory_actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trajectory_step_count: Option<u32>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalReport {
    pub summary: EvalSummary,
    pub results: Vec<EvalResult>,
}

impl EvalReport {
    pub fn from_results(results: Vec<EvalResult>) -> Self {
        let summary = EvalSummary::from_results(&results);
        Self { summary, results }
    }
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

// --- Regression comparison types ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalComparison {
    pub pass_rate_delta: f64,
    pub avg_elapsed_delta_ms: f64,
    pub total_token_delta: Option<i64>,
    pub regressions: Vec<ScenarioRegression>,
    pub improvements: Vec<ScenarioImprovement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioRegression {
    pub scenario_id: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioImprovement {
    pub scenario_id: String,
    pub kind: String,
}

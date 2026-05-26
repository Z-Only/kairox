//! Headless evaluation harness for Kairox.

use agent_config::Config;
use agent_core::{
    AppFacade, DomainEvent, EventPayload, SendMessageRequest, StartSessionRequest, TraceEntry,
};
use agent_memory::SqliteMemoryStore;
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

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

pub struct EvalHarness {
    runtime: Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    workspace_id: agent_core::WorkspaceId,
    options: EvalRunOptions,
    default_profile: String,
}

impl EvalHarness {
    pub async fn new(options: EvalRunOptions) -> Result<Self> {
        let mut config = match options.config.clone() {
            Some(config) => config,
            None => Config::load_with_project_root(Some(&options.workspace_root))?,
        };
        if !options.enable_hooks {
            config.features.hooks = false;
            config.hooks.clear();
        }
        let default_profile = options
            .default_profile
            .clone()
            .unwrap_or_else(|| config.default_profile());
        let router = config.build_router();
        let ollama_clients = agent_config::build_ollama_clients(&config);
        let mcp_server_defs = if options.enable_mcp {
            config.mcp_server_defs()
        } else {
            Vec::new()
        };
        let config_arc = Arc::new(config);

        let store = SqliteEventStore::in_memory().await?;
        let mem_store = Arc::new(SqliteMemoryStore::new(store.pool().clone()).await?)
            as Arc<dyn agent_memory::MemoryStore>;
        let runtime = LocalRuntime::new(store, router)
            .with_approval_and_sandbox(options.approval_policy, options.sandbox_policy.clone())
            .with_context_limit(100_000)
            .with_memory_store(mem_store)
            .with_config(config_arc)
            .with_ollama_clients(ollama_clients)
            .with_builtin_tools(options.workspace_root.clone())
            .await
            .with_mcp_servers(mcp_server_defs)
            .await;
        let runtime = Arc::new(runtime);
        let workspace_id = runtime
            .open_workspace(options.workspace_root.display().to_string())
            .await?
            .workspace_id;

        Ok(Self {
            runtime,
            workspace_id,
            options,
            default_profile,
        })
    }

    pub async fn run_scenario(&mut self, scenario: &EvalScenario) -> Result<EvalResult> {
        let profile = scenario
            .profile
            .clone()
            .unwrap_or_else(|| self.default_profile.clone());
        let approval = scenario
            .approval_policy
            .unwrap_or(self.options.approval_policy);
        let sandbox = scenario
            .sandbox_policy
            .clone()
            .unwrap_or_else(|| self.options.sandbox_policy.clone());

        self.runtime.set_approval_policy(approval).await;
        self.runtime.set_sandbox_policy(sandbox.clone()).await;

        let session_id = self
            .runtime
            .start_session(StartSessionRequest {
                workspace_id: self.workspace_id.clone(),
                model_profile: profile.clone(),
                approval_policy: Some(approval.to_string()),
                sandbox_policy: Some(serde_json::to_string(&sandbox)?),
            })
            .await?;

        let started = Instant::now();
        let send_result = self
            .runtime
            .send_message(SendMessageRequest {
                workspace_id: self.workspace_id.clone(),
                session_id: session_id.clone(),
                content: scenario.prompt.clone(),
                attachments: Vec::new(),
            })
            .await;
        let elapsed_ms = started.elapsed().as_millis().max(1) as u64;

        let projection = self
            .runtime
            .get_session_projection(session_id.clone())
            .await?;
        let trace_entries = self.runtime.get_trace(session_id).await?;
        let trace_events: Vec<DomainEvent> = trace_entries.into_iter().map(trace_event).collect();
        let event_types: Vec<String> = trace_events
            .iter()
            .map(|event| event.event_type.clone())
            .collect();
        let assistant_response = projection
            .messages
            .iter()
            .rev()
            .find(|message| {
                matches!(
                    message.role,
                    agent_core::projection::ProjectedRole::Assistant
                )
            })
            .map(|message| message.content.clone());
        let tool_invocations = count_events(&trace_events, |payload| {
            matches!(payload, EventPayload::ToolInvocationStarted { .. })
        });
        let tool_failures = count_events(&trace_events, |payload| {
            matches!(payload, EventPayload::ToolInvocationFailed { .. })
        });
        let context_input_tokens = projection
            .last_context_usage
            .as_ref()
            .map(|usage| usage.total_tokens);
        let context_window = projection
            .last_context_usage
            .as_ref()
            .map(|usage| usage.context_window);

        let mut failures = Vec::new();
        if let Err(error) = send_result {
            failures.push(format!("runtime error: {error}"));
        }
        evaluate_expectations(
            &scenario.expected,
            assistant_response.as_deref(),
            &event_types,
            tool_invocations,
            tool_failures,
            &mut failures,
        );
        let error = failures
            .iter()
            .find_map(|failure| failure.strip_prefix("runtime error: ").map(str::to_string));

        Ok(EvalResult {
            scenario_id: scenario.id.clone(),
            profile,
            passed: failures.is_empty(),
            failures,
            error,
            elapsed_ms,
            assistant_response,
            event_types,
            tool_invocations,
            tool_failures,
            context_input_tokens,
            context_window,
            trace: self.options.include_trace.then_some(trace_events),
        })
    }

    pub async fn run_scenarios(&mut self, scenarios: &[EvalScenario]) -> Vec<EvalResult> {
        let mut results = Vec::with_capacity(scenarios.len());
        for scenario in scenarios {
            match self.run_scenario(scenario).await {
                Ok(result) => results.push(result),
                Err(error) => results.push(EvalResult {
                    scenario_id: scenario.id.clone(),
                    profile: scenario
                        .profile
                        .clone()
                        .unwrap_or_else(|| self.default_profile.clone()),
                    passed: false,
                    failures: vec![error.to_string()],
                    error: Some(error.to_string()),
                    ..EvalResult::default()
                }),
            }
        }
        results
    }
}

pub fn load_scenarios(path: impl AsRef<Path>) -> Result<Vec<EvalScenario>> {
    let content = std::fs::read_to_string(path)?;
    load_scenarios_from_str(&content)
}

pub fn load_scenarios_from_str(input: &str) -> Result<Vec<EvalScenario>> {
    let mut scenarios = Vec::new();
    for (index, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let scenario =
            serde_json::from_str(trimmed).map_err(|source| EvalError::ScenarioParse {
                line: index + 1,
                source,
            })?;
        scenarios.push(scenario);
    }
    Ok(scenarios)
}

pub fn write_results_jsonl(path: impl AsRef<Path>, results: &[EvalResult]) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let mut output = String::new();
    for result in results {
        output.push_str(&serde_json::to_string(result)?);
        output.push('\n');
    }
    std::fs::write(path, output)?;
    Ok(())
}

pub fn write_summary_json(path: impl AsRef<Path>, summary: &EvalSummary) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, serde_json::to_string_pretty(summary)?)?;
    Ok(())
}

fn evaluate_expectations(
    expected: &EvalExpectation,
    assistant_response: Option<&str>,
    event_types: &[String],
    tool_invocations: usize,
    tool_failures: usize,
    failures: &mut Vec<String>,
) {
    for needle in &expected.assistant_contains {
        match assistant_response {
            Some(response) if response.contains(needle) => {}
            Some(_) => failures.push(format!("assistant response missing substring: {needle}")),
            None => failures.push(format!("assistant response missing substring: {needle}")),
        }
    }

    for event_type in &expected.event_types {
        if !event_types.iter().any(|seen| seen == event_type) {
            failures.push(format!("missing event type: {event_type}"));
        }
    }

    if let Some(minimum) = expected.min_tool_invocations {
        if tool_invocations < minimum {
            failures.push(format!(
                "tool invocations below minimum: expected at least {minimum}, got {tool_invocations}"
            ));
        }
    }

    if let Some(maximum) = expected.max_tool_failures {
        if tool_failures > maximum {
            failures.push(format!(
                "tool failures above maximum: expected at most {maximum}, got {tool_failures}"
            ));
        }
    }
}

fn count_events(events: &[DomainEvent], predicate: impl Fn(&EventPayload) -> bool) -> usize {
    events
        .iter()
        .filter(|event| predicate(&event.payload))
        .count()
}

fn trace_event(entry: TraceEntry) -> DomainEvent {
    entry.event
}

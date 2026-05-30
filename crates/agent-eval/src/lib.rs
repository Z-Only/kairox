//! Headless evaluation harness for Kairox.

use agent_config::Config;
use agent_core::{
    AgentId, AppFacade, DomainEvent, EventPayload, PrivacyClassification, SendMessageRequest,
    SessionId, StartSessionRequest, TraceEntry, WorkspaceId,
};
use agent_memory::SqliteMemoryStore;
use agent_models::{FakeModelClient, ModelRouter};
use agent_runtime::LocalRuntime;
use agent_store::{EventStore, SqliteEventStore};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod types;

pub use types::{
    EvalError, EvalExpectation, EvalResult, EvalRunOptions, EvalScenario, EvalSummary, Result,
};

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
        if let Some(threshold) = options.auto_compact_threshold {
            config.context.auto_compact_threshold = threshold;
        }
        let default_profile = options
            .default_profile
            .clone()
            .unwrap_or_else(|| config.default_profile());
        let mut router = config.build_router();
        if options.fake_emit_tool_call {
            install_fake_tool_call(&mut router, &options)?;
        }
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

        if let Some(pairs) = self.options.seed_synthetic_pairs {
            seed_synthetic_history_pairs(
                self.runtime.store(),
                &self.workspace_id,
                &session_id,
                pairs,
            )
            .await?;
        }

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
        let trace_events: Vec<DomainEvent> = match self.options.wait_timeout_ms {
            Some(timeout_ms) if !scenario.expected.event_types.is_empty() => {
                wait_for_expected_event_types(
                    &self.runtime,
                    session_id,
                    &scenario.expected.event_types,
                    Duration::from_millis(timeout_ms),
                )
                .await?
            }
            _ => {
                let trace_entries = self.runtime.get_trace(session_id).await?;
                trace_entries.into_iter().map(trace_event).collect()
            }
        };
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

pub fn filter_scenarios_by_tags(
    scenarios: &[EvalScenario],
    include_tags: &[String],
    exclude_tags: &[String],
) -> Vec<EvalScenario> {
    scenarios
        .iter()
        .filter(|scenario| scenario_matches_tags(scenario, include_tags, exclude_tags))
        .cloned()
        .collect()
}

fn scenario_matches_tags(
    scenario: &EvalScenario,
    include_tags: &[String],
    exclude_tags: &[String],
) -> bool {
    let has_included_tag = include_tags.is_empty()
        || include_tags
            .iter()
            .any(|tag| scenario.tags.iter().any(|candidate| candidate == tag));
    let has_excluded_tag = exclude_tags
        .iter()
        .any(|tag| scenario.tags.iter().any(|candidate| candidate == tag));
    has_included_tag && !has_excluded_tag
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

/// Re-register the `fake` profile on `router` with a [`FakeModelClient`]
/// that also emits a tool-call after its token stream. The default tool
/// is `fs.list {"path":"."}` which is always callable in a temp
/// workspace; callers can override via `options.fake_tool_id` and
/// `options.fake_tool_arguments`.
fn install_fake_tool_call(router: &mut ModelRouter, options: &EvalRunOptions) -> Result<()> {
    let profile = router.get_profile("fake").cloned().ok_or_else(|| {
        EvalError::Cli(
            "--fake-emit-tool-call requested but the loaded config has no `fake` profile"
                .to_string(),
        )
    })?;
    let tool_id = options
        .fake_tool_id
        .clone()
        .unwrap_or_else(|| "fs.list".to_string());
    let arguments = options
        .fake_tool_arguments
        .clone()
        .unwrap_or_else(|| serde_json::json!({"path": "."}));
    let client = FakeModelClient::new(vec!["hello from Kairox".to_string()])
        .with_tool_call_for(tool_id, arguments);
    router.register(profile, Arc::new(client));
    Ok(())
}

/// Append `pairs` synthetic `UserMessageAdded`/`AssistantMessageCompleted`
/// turns directly into the event store. Used to give the auto-compaction
/// scheduler enough history (`>= KEEP_LAST_PAIRS + 1` pairs) so that
/// `compact_session` actually emits `ContextCompactionStarted` and
/// `ContextCompactionCompleted` on the first real turn. Timestamps are
/// monotonic and live in the recent past so the boundary picker keeps
/// them in the compaction candidate window.
async fn seed_synthetic_history_pairs(
    store: &SqliteEventStore,
    workspace_id: &WorkspaceId,
    session_id: &SessionId,
    pairs: usize,
) -> Result<()> {
    let base = chrono::Utc::now() - chrono::Duration::hours(1);
    for i in 0..pairs {
        let user = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::UserMessageAdded {
                message_id: format!("eval-seed-user-{i}"),
                content: format!("seed user {i}"),
            },
        )
        .with_timestamp(base + chrono::Duration::seconds(i as i64 * 2));
        store.append(&user).await?;

        let assistant = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::AssistantMessageCompleted {
                message_id: format!("eval-seed-assistant-{i}"),
                content: format!("seed assistant {i}"),
            },
        )
        .with_timestamp(base + chrono::Duration::seconds(i as i64 * 2 + 1));
        store.append(&assistant).await?;
    }
    Ok(())
}

/// Poll [`AppFacade::get_trace`] up to `timeout` waiting for every event
/// type in `expected_types` to appear at least once. Returns the most
/// recently observed trace. Used by [`EvalHarness::run_scenario`] to
/// surface events emitted by detached background tasks (e.g.
/// auto-compaction) that fire after `send_message` has returned.
async fn wait_for_expected_event_types(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    session_id: agent_core::SessionId,
    expected_types: &[String],
    timeout: Duration,
) -> Result<Vec<DomainEvent>> {
    let deadline = Instant::now() + timeout;
    let poll_interval = Duration::from_millis(25);
    loop {
        let trace_entries = runtime.get_trace(session_id.clone()).await?;
        let trace_events: Vec<DomainEvent> = trace_entries.into_iter().map(trace_event).collect();
        let event_types: Vec<&str> = trace_events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect();
        let all_present = expected_types
            .iter()
            .all(|needle| event_types.iter().any(|seen| *seen == needle));
        if all_present || Instant::now() >= deadline {
            return Ok(trace_events);
        }
        tokio::time::sleep(poll_interval).await;
    }
}

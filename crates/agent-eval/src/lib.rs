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
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod types;
#[cfg(test)]
#[path = "types_tests.rs"]
mod types_tests;

pub use types::{
    EvalComparison, EvalError, EvalExpectation, EvalReport, EvalResult, EvalRunOptions,
    EvalScenario, EvalSummary, Result, ScenarioImprovement, ScenarioRegression,
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

        // First turn
        let send_result = self
            .runtime
            .send_message(SendMessageRequest {
                workspace_id: self.workspace_id.clone(),
                session_id: session_id.clone(),
                content: scenario.prompt.clone(),
                display_content: None,
                attachments: Vec::new(),
            })
            .await;

        // Additional turns (with retry for session-busy during compaction)
        let mut multi_turn_errors: Vec<String> = Vec::new();
        for turn_prompt in &scenario.turns {
            let mut attempts = 0;
            loop {
                match self
                    .runtime
                    .send_message(SendMessageRequest {
                        workspace_id: self.workspace_id.clone(),
                        session_id: session_id.clone(),
                        content: turn_prompt.clone(),
                        display_content: None,
                        attachments: Vec::new(),
                    })
                    .await
                {
                    Ok(()) => break,
                    Err(e) if attempts < 10 && e.to_string().contains("busy") => {
                        attempts += 1;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(e) => {
                        multi_turn_errors.push(format!("multi-turn error: {e}"));
                        break;
                    }
                }
            }
        }

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

        let turns_count = count_events(&trace_events, |payload| {
            matches!(payload, EventPayload::AssistantMessageCompleted { .. })
        });

        // Extract trajectory actions from tool invocation events
        let trajectory_actions: Vec<String> = trace_events
            .iter()
            .filter_map(|event| match &event.payload {
                EventPayload::ToolInvocationStarted { tool_id, .. } => Some(tool_id.clone()),
                _ => None,
            })
            .collect();

        let mut failures = Vec::new();
        if let Err(error) = send_result {
            failures.push(format!("runtime error: {error}"));
        }
        failures.extend(multi_turn_errors);
        evaluate_expectations(
            &scenario.expected,
            ExpectationObservation {
                assistant_response: assistant_response.as_deref(),
                event_types: &event_types,
                tool_invocations,
                tool_failures,
                elapsed_ms,
                context_input_tokens,
                turns_count,
                trajectory_actions: &trajectory_actions,
            },
            &mut failures,
        )?;
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
            turns_count,
            trajectory_actions,
            trajectory_step_count: Some(tool_invocations as u32),
        })
    }

    pub async fn run_scenarios(&mut self, scenarios: &[EvalScenario]) -> Vec<EvalResult> {
        self.run_scenarios_with_mode(scenarios, false).await
    }

    pub async fn run_scenarios_until_failure(
        &mut self,
        scenarios: &[EvalScenario],
    ) -> Vec<EvalResult> {
        self.run_scenarios_with_mode(scenarios, true).await
    }

    async fn run_scenarios_with_mode(
        &mut self,
        scenarios: &[EvalScenario],
        fail_fast: bool,
    ) -> Vec<EvalResult> {
        let mut results = Vec::with_capacity(scenarios.len());
        for scenario in scenarios {
            match self.run_scenario(scenario).await {
                Ok(result) => results.push(result),
                Err(error) => {
                    let err_str = error.to_string();
                    results.push(EvalResult {
                        scenario_id: scenario.id.clone(),
                        profile: scenario
                            .profile
                            .clone()
                            .unwrap_or_else(|| self.default_profile.clone()),
                        passed: false,
                        failures: vec![err_str.clone()],
                        error: Some(err_str),
                        elapsed_ms: 0,
                        assistant_response: None,
                        event_types: Vec::new(),
                        tool_invocations: 0,
                        tool_failures: 0,
                        context_input_tokens: None,
                        context_window: None,
                        trace: None,
                        turns_count: 0,
                        trajectory_actions: Vec::new(),
                        trajectory_step_count: None,
                    });
                }
            };
            let failed = results.last().is_some_and(|result| !result.passed);
            if fail_fast && failed {
                break;
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
    ensure_parent_dir(path.as_ref())?;
    let mut output = String::new();
    for result in results {
        output.push_str(&serde_json::to_string(result)?);
        output.push('\n');
    }
    std::fs::write(path, output)?;
    Ok(())
}

pub fn write_summary_json(path: impl AsRef<Path>, summary: &EvalSummary) -> Result<()> {
    ensure_parent_dir(path.as_ref())?;
    std::fs::write(path, serde_json::to_string_pretty(summary)?)?;
    Ok(())
}

pub fn write_report_json(path: impl AsRef<Path>, report: &EvalReport) -> Result<()> {
    ensure_parent_dir(path.as_ref())?;
    std::fs::write(path, serde_json::to_string_pretty(report)?)?;
    Ok(())
}

pub fn write_comparison_json(path: impl AsRef<Path>, comparison: &EvalComparison) -> Result<()> {
    ensure_parent_dir(path.as_ref())?;
    std::fs::write(path, serde_json::to_string_pretty(comparison)?)?;
    Ok(())
}

pub fn compare_reports(baseline: &EvalReport, candidate: &EvalReport) -> EvalComparison {
    let pass_rate_delta = candidate.summary.success_rate - baseline.summary.success_rate;
    let avg_elapsed_delta_ms = candidate.summary.avg_elapsed_ms - baseline.summary.avg_elapsed_ms;
    let total_token_delta = match (
        candidate.summary.total_context_input_tokens,
        baseline.summary.total_context_input_tokens,
    ) {
        (Some(c), Some(b)) => Some(c as i64 - b as i64),
        _ => None,
    };

    let baseline_map: HashMap<&str, &EvalResult> = baseline
        .results
        .iter()
        .map(|r| (r.scenario_id.as_str(), r))
        .collect();
    let candidate_map: HashMap<&str, &EvalResult> = candidate
        .results
        .iter()
        .map(|r| (r.scenario_id.as_str(), r))
        .collect();

    let mut regressions = Vec::new();
    let mut improvements = Vec::new();

    for (id, cand) in &candidate_map {
        if let Some(base) = baseline_map.get(id) {
            if base.passed && !cand.passed {
                regressions.push(ScenarioRegression {
                    scenario_id: id.to_string(),
                    kind: "passed_to_failed".into(),
                });
            } else if !base.passed && cand.passed {
                improvements.push(ScenarioImprovement {
                    scenario_id: id.to_string(),
                    kind: "failed_to_passed".into(),
                });
            }

            if base.elapsed_ms > 0 && cand.elapsed_ms > base.elapsed_ms {
                let pct =
                    ((cand.elapsed_ms - base.elapsed_ms) as f64 / base.elapsed_ms as f64) * 100.0;
                if pct > 50.0 {
                    regressions.push(ScenarioRegression {
                        scenario_id: id.to_string(),
                        kind: format!("slower_by_{:.0}%", pct),
                    });
                }
            } else if base.elapsed_ms > 0 && cand.elapsed_ms < base.elapsed_ms {
                let pct =
                    ((base.elapsed_ms - cand.elapsed_ms) as f64 / base.elapsed_ms as f64) * 100.0;
                if pct > 50.0 {
                    improvements.push(ScenarioImprovement {
                        scenario_id: id.to_string(),
                        kind: format!("faster_by_{:.0}%", pct),
                    });
                }
            }

            if let (Some(c_tok), Some(b_tok)) =
                (cand.context_input_tokens, base.context_input_tokens)
            {
                if b_tok > 0 && c_tok > b_tok {
                    let pct = ((c_tok - b_tok) as f64 / b_tok as f64) * 100.0;
                    if pct > 50.0 {
                        regressions.push(ScenarioRegression {
                            scenario_id: id.to_string(),
                            kind: format!("more_tokens_{:.0}%", pct),
                        });
                    }
                }
            }
        }
    }

    EvalComparison {
        pass_rate_delta,
        avg_elapsed_delta_ms,
        total_token_delta,
        regressions,
        improvements,
    }
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn evaluate_expectations(
    expected: &EvalExpectation,
    observed: ExpectationObservation<'_>,
    failures: &mut Vec<String>,
) -> Result<()> {
    for needle in &expected.assistant_contains {
        match observed.assistant_response {
            Some(response) if response.contains(needle) => {}
            Some(_) => failures.push(format!("assistant response missing substring: {needle}")),
            None => failures.push(format!("assistant response missing substring: {needle}")),
        }
    }

    for needle in &expected.assistant_not_contains {
        if let Some(response) = observed.assistant_response {
            if response.contains(needle) {
                failures.push(format!(
                    "assistant response contains forbidden substring: {needle}"
                ));
            }
        }
    }

    for pattern in &expected.assistant_matches_regex {
        let re = regex::Regex::new(pattern).map_err(|source| EvalError::Regex {
            pattern: pattern.clone(),
            source,
        })?;
        match observed.assistant_response {
            Some(response) if re.is_match(response) => {}
            Some(_) => failures.push(format!(
                "assistant response does not match regex: {pattern}"
            )),
            None => failures.push(format!(
                "assistant response does not match regex: {pattern}"
            )),
        }
    }

    for event_type in &expected.event_types {
        if !observed.event_types.iter().any(|seen| seen == event_type) {
            failures.push(format!("missing event type: {event_type}"));
        }
    }

    for event_type in &expected.forbidden_event_types {
        if observed.event_types.iter().any(|seen| seen == event_type) {
            failures.push(format!("forbidden event type present: {event_type}"));
        }
    }

    for (event_type, minimum) in &expected.min_events_of_type {
        let count = observed
            .event_types
            .iter()
            .filter(|seen| *seen == event_type)
            .count();
        if count < *minimum {
            failures.push(format!(
                "event type `{event_type}` count below minimum: expected at least {minimum}, got {count}"
            ));
        }
    }

    for (event_type, maximum) in &expected.max_events_of_type {
        let count = observed
            .event_types
            .iter()
            .filter(|seen| *seen == event_type)
            .count();
        if count > *maximum {
            failures.push(format!(
                "event type `{event_type}` count above maximum: expected at most {maximum}, got {count}"
            ));
        }
    }

    if let Some(minimum) = expected.min_tool_invocations {
        if observed.tool_invocations < minimum {
            failures.push(format!(
                "tool invocations below minimum: expected at least {minimum}, got {}",
                observed.tool_invocations
            ));
        }
    }

    if let Some(maximum) = expected.max_tool_failures {
        if observed.tool_failures > maximum {
            failures.push(format!(
                "tool failures above maximum: expected at most {maximum}, got {}",
                observed.tool_failures
            ));
        }
    }

    if let Some(maximum) = expected.max_elapsed_ms {
        if observed.elapsed_ms > maximum {
            failures.push(format!(
                "elapsed time above maximum: expected at most {maximum} ms, got {} ms",
                observed.elapsed_ms
            ));
        }
    }

    if let Some(maximum) = expected.max_context_input_tokens {
        match observed.context_input_tokens {
            Some(tokens) if tokens <= maximum => {}
            Some(tokens) => failures.push(format!(
                "context input tokens above maximum: expected at most {maximum}, got {tokens}"
            )),
            None => failures.push("context input tokens unavailable".into()),
        }
    }

    if let Some(max_turns) = expected.max_turns {
        if observed.turns_count > max_turns {
            failures.push(format!(
                "turns above maximum: expected at most {max_turns}, got {}",
                observed.turns_count
            ));
        }
    }

    if !expected.trajectory_actions.is_empty() {
        for (i, expected_action) in expected.trajectory_actions.iter().enumerate() {
            match observed.trajectory_actions.get(i) {
                Some(actual) if actual == expected_action => {}
                Some(actual) => failures.push(format!(
                    "trajectory action at step {i}: expected `{expected_action}`, got `{actual}`"
                )),
                None => failures.push(format!(
                    "trajectory action at step {i}: expected `{expected_action}`, but only {} steps recorded",
                    observed.trajectory_actions.len()
                )),
            }
        }
    }

    if let Some(max_steps) = expected.max_trajectory_steps {
        let actual = observed.trajectory_actions.len() as u32;
        if actual > max_steps {
            failures.push(format!(
                "trajectory steps above maximum: expected at most {max_steps}, got {actual}"
            ));
        }
    }

    Ok(())
}

struct ExpectationObservation<'a> {
    assistant_response: Option<&'a str>,
    event_types: &'a [String],
    tool_invocations: usize,
    tool_failures: usize,
    elapsed_ms: u64,
    context_input_tokens: Option<u64>,
    turns_count: usize,
    trajectory_actions: &'a [String],
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

fn install_fake_tool_call(router: &mut ModelRouter, options: &EvalRunOptions) -> Result<()> {
    let profile = router.get_profile("fake").ok_or_else(|| {
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
                display_content: None,
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

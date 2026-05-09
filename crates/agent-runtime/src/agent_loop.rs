//! Agent loop logic extracted from the runtime facade.
//!
//! This module contains the core orchestrating loop that drives the
//! model → tool-call → permission → execute → feed-back cycle, as well as
//! the helper that converts session history into model messages.

use crate::event_emitter::append_and_broadcast;
use crate::skills::render_active_skill_block;
use crate::task_graph::TaskGraph;
use agent_core::{
    AgentId, AgentRole, DomainEvent, EventPayload, PermissionDecision, PrivacyClassification,
    SendMessageRequest, TaskId,
};
use agent_memory::{strip_memory_markers, MemoryStore};
use agent_models::{ModelClient, ModelEvent, ModelRequest, ToolCall};
use agent_store::EventStore;
use agent_tools::{PermissionEngine, ToolInvocation, ToolRegistry};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub const SYSTEM_PROMPT: &str = "\
You are Kairox, a helpful AI assistant with memory capabilities.\n\n\
## Memory Protocol\n\
When you learn something worth remembering about the user or workspace, \
use <memory> tags to save it. Examples:\n\
- <memory scope=\"session\">Temporary note for this session</memory>\n\
- <memory scope=\"user\" key=\"preferred-language\">User prefers Rust</memory>\n\
- <memory scope=\"workspace\" key=\"build-cmd\">Use cargo nextest</memory>\n\n\
Guidelines:\n\
- Use scope=\"session\" for temporary notes (auto-accepted)\n\
- Use scope=\"user\" for user preferences (requires approval)\n\
- Use scope=\"workspace\" for project settings (requires approval)\n\
- Always include a key when using user or workspace scope\n\
- You may include multiple <memory> tags in one response\n\
- The <memory> tags will be stripped from displayed output, so also state \
the information naturally in your response.\n\
";

pub const MAX_AGENT_LOOP_ITERATIONS: usize = 20;

pub fn build_model_messages(
    user_content: &str,
    session_events: &[DomainEvent],
) -> Vec<agent_models::ModelMessage> {
    let mut messages = Vec::new();
    // Collect tool call info from ModelToolCallRequested events so we can
    // populate the tool_calls field on assistant messages. We group them
    // by the preceding AssistantMessageCompleted event.
    let mut pending_tool_calls: Vec<agent_models::ToolCall> = Vec::new();
    let mut tool_results: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new(); // tool_call_id -> (tool_id, output_preview)

    // P2: Compute the union of timestamp ranges covered by CompactionSummary
    // events. Real events whose timestamp falls inside ANY covered range are
    // skipped, and the corresponding summary text is injected as a
    // pseudo-user message at the position the first replaced event would
    // have occupied. Summaries themselves are never emitted as plain events.
    let mut summaries: Vec<(
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
        String,
    )> = session_events
        .iter()
        .filter_map(|e| match &e.payload {
            EventPayload::CompactionSummary {
                replaces_event_range: (first, last),
                content,
                ..
            } => Some((*first, *last, content.clone())),
            _ => None,
        })
        .collect();
    summaries.sort_by_key(|(first, _, _)| *first);
    let covered = |ts: chrono::DateTime<chrono::Utc>| -> bool {
        summaries
            .iter()
            .any(|(first, last, _)| ts >= *first && ts <= *last)
    };

    // First pass: collect tool call requests and results — skip events that
    // fall inside a covered range so the summary fully replaces them.
    for event in session_events {
        if matches!(event.payload, EventPayload::CompactionSummary { .. }) {
            continue;
        }
        if covered(event.timestamp) {
            continue;
        }
        match &event.payload {
            EventPayload::ModelToolCallRequested {
                tool_call_id,
                tool_id,
            } => {
                pending_tool_calls.push(agent_models::ToolCall {
                    id: tool_call_id.clone(),
                    name: tool_id.clone(),
                    arguments: serde_json::json!({}),
                });
            }
            EventPayload::ToolInvocationCompleted {
                invocation_id,
                tool_id,
                output_preview,
                ..
            } => {
                tool_results.insert(
                    invocation_id.clone(),
                    (tool_id.clone(), output_preview.clone()),
                );
            }
            _ => {}
        }
    }

    // Second pass: build messages with proper tool_calls and tool_call_id.
    // Summaries are injected just before the first event whose timestamp
    // is strictly greater than the summary's `last_ts` (so they appear
    // chronologically in place of the replaced range).
    let mut injected: Vec<bool> = vec![false; summaries.len()];
    let mut tool_call_idx = 0;
    for event in session_events {
        if matches!(event.payload, EventPayload::CompactionSummary { .. }) {
            continue;
        }
        // Inject any summary whose covered range ends strictly before this event.
        for (idx, (_, last_ts, content)) in summaries.iter().enumerate() {
            if !injected[idx] && event.timestamp > *last_ts {
                messages.push(agent_models::ModelMessage {
                    role: "user".into(),
                    content: format!("[Conversation summary]\n{content}"),
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                });
                injected[idx] = true;
            }
        }
        if covered(event.timestamp) {
            continue;
        }
        match &event.payload {
            EventPayload::UserMessageAdded { content, .. } => {
                messages.push(agent_models::ModelMessage {
                    role: "user".into(),
                    content: content.clone(),
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                });
            }
            EventPayload::AssistantMessageCompleted { content, .. } => {
                // Gather tool calls that were requested between this assistant
                // message and the next one (or the end of events). Tool calls
                // in pending_tool_calls are in order from the first pass.
                let mut tc_for_msg = Vec::new();
                while tool_call_idx < pending_tool_calls.len() {
                    tc_for_msg.push(pending_tool_calls[tool_call_idx].clone());
                    tool_call_idx += 1;
                    // If there are more tool calls, they belong to this same
                    // assistant turn (models can request multiple tools at once).
                    // We can\'t easily determine where the current assistant\'s
                    // tool calls end from just session events, so we assign
                    // all pending tool calls that follow to the most recent
                    // assistant message. This works because in a single agent
                    // loop iteration, all tool calls come from one model response.
                    //
                    // For multi-iteration support, we\'d need to track which
                    // iteration each tool call belongs to, but the current
                    // runtime only uses build_model_messages for the initial
                    // request — subsequent iterations build messages directly
                    // from current_request.
                    //
                    // For now: only assign tool calls to the LAST assistant message.
                    // We\'ll fix this after the loop.
                }
                // Don\'t add yet — we need to know if this is the last assistant
                // message to properly assign tool calls. For simplicity, we
                // always append tool calls to the last assistant message.
                // Instead, store tool calls separately and attach them below.
                messages.push(agent_models::ModelMessage {
                    role: "assistant".into(),
                    content: content.clone(),
                    tool_calls: Vec::new(), // will be fixed below
                    tool_call_id: None,
                });
            }
            EventPayload::ToolInvocationCompleted {
                invocation_id,
                output_preview,
                ..
            } => {
                // Use tool_call_id from the invocation_id to link back to the tool call
                messages.push(agent_models::ModelMessage {
                    role: "tool".into(),
                    content: output_preview.clone(),
                    tool_calls: Vec::new(),
                    tool_call_id: Some(invocation_id.clone()),
                });
            }
            EventPayload::ToolInvocationFailed {
                invocation_id,
                error,
                ..
            } => {
                messages.push(agent_models::ModelMessage {
                    role: "tool".into(),
                    content: format!("Error: {}", error),
                    tool_calls: Vec::new(),
                    tool_call_id: Some(invocation_id.clone()),
                });
            }
            _ => {}
        }
    }

    // Attach all collected tool calls to the last assistant message.
    // In the agent loop, after a model response with tool calls, the
    // AssistantMessageCompleted is emitted, then tool results follow.
    // All pending tool calls belong to the most recent assistant turn.
    if !pending_tool_calls.is_empty() {
        if let Some(last_assistant) = messages.iter_mut().rev().find(|m| m.role == "assistant") {
            // Only attach tool calls that haven\'t already been consumed
            // (i.e., tool calls where the corresponding tool results appear
            // after this assistant message in the conversation)
            last_assistant.tool_calls = pending_tool_calls;
        }
    }

    if messages.is_empty() || messages.last().map(|m| m.content.as_str()) != Some(user_content) {
        messages.push(agent_models::ModelMessage {
            role: "user".into(),
            content: user_content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        });
    }
    messages
}

/// Bundles every dependency `run_agent_loop` needs. Introduced to avoid a
/// 12-argument signature once `config` and `session_states` were added in
/// Task 8.
pub struct AgentLoopDeps<'a, S, M>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    pub store: &'a Arc<S>,
    pub model: &'a Arc<M>,
    pub event_tx: &'a tokio::sync::broadcast::Sender<DomainEvent>,
    pub tool_registry: &'a Arc<Mutex<ToolRegistry>>,
    pub permission_engine: &'a Arc<Mutex<PermissionEngine>>,
    pub pending_permissions:
        &'a Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>,
    pub memory_store: &'a Option<Arc<dyn MemoryStore>>,
    pub task_graphs: &'a Arc<Mutex<HashMap<String, TaskGraph>>>,
    pub active_cancellation: &'a Arc<Mutex<Option<CancellationToken>>>,
    pub config: &'a Arc<agent_config::Config>,
    pub session_states: &'a Arc<Mutex<HashMap<String, crate::session::SessionState>>>,
    pub skill_registry: &'a Option<Arc<dyn agent_skills::SkillRegistry>>,
    pub active_skills: &'a Arc<Mutex<HashMap<String, Vec<String>>>>,
}

/// Resolve the active model profile alias from a session's full event log.
///
/// Priority (newest to oldest wins):
/// 1. The most recent `EventPayload::ModelProfileSwitched.to_profile`.
/// 2. `EventPayload::SessionInitialized.model_profile` (the session's
///    original profile).
/// 3. The literal `"fake"` (only reached for broken event logs — kept for
///    symmetry with the pre-P4 fallback).
async fn load_active_skill_blocks(
    skill_registry: &Option<Arc<dyn agent_skills::SkillRegistry>>,
    active_skills: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    session_id: &agent_core::SessionId,
) -> agent_core::Result<Vec<String>> {
    let Some(registry) = skill_registry else {
        return Ok(Vec::new());
    };
    let skill_ids = {
        let active_skills = active_skills.lock().await;
        active_skills
            .get(&session_id.to_string())
            .cloned()
            .unwrap_or_default()
    };

    let mut rendered_skills = Vec::new();
    for skill_id in skill_ids {
        let skill_id_value = agent_skills::SkillId::new(skill_id.clone());
        let document = match registry.load_document(&skill_id_value).await {
            Ok(document) => document,
            Err(error) => {
                tracing::warn!(
                    skill_id = %skill_id,
                    error = %error,
                    "skipping active skill because its document could not be loaded"
                );
                continue;
            }
        };
        let source = crate::skills::skill_source_kind_to_string(document.metadata.source.kind);
        rendered_skills.push(render_active_skill_block(
            &document.metadata.name,
            &source,
            &document.body_markdown,
        ));
    }

    Ok(rendered_skills)
}

pub(crate) fn latest_model_profile_for(events: &[agent_core::DomainEvent]) -> String {
    for event in events.iter().rev() {
        match &event.payload {
            agent_core::EventPayload::ModelProfileSwitched { to_profile, .. } => {
                return to_profile.clone();
            }
            agent_core::EventPayload::SessionInitialized { model_profile } => {
                return model_profile.clone();
            }
            _ => {}
        }
    }
    "fake".to_string()
}

pub async fn run_agent_loop<S, M>(
    deps: AgentLoopDeps<'_, S, M>,
    request: &SendMessageRequest,
) -> agent_core::Result<()>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    // Destructure once so the existing body keeps working with the same
    // local names (no rename needed).
    let AgentLoopDeps {
        store,
        model,
        event_tx,
        tool_registry,
        permission_engine,
        pending_permissions,
        memory_store,
        task_graphs,
        active_cancellation,
        config,
        session_states,
        skill_registry,
        active_skills,
    } = deps;
    // Record user message
    let user_event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::UserMessageAdded {
            message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
            content: request.content.clone(),
        },
    );
    append_and_broadcast(&**store, event_tx, &user_event).await?;

    // Load session history for context
    let session_events = store
        .load_session(&request.session_id)
        .await
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

    // Resolve the profile alias from session events (fallback "fake" for legacy).
    // Uses the shared `latest_model_profile_for` helper so mid-session
    // `ModelProfileSwitched` events take effect on the very next iteration.
    let model_profile_alias: String = latest_model_profile_for(&session_events);

    // Resolve ModelLimits: prefer per-session cached limits (Task 10's probe may
    // have refined them), otherwise re-resolve from config + registry.
    let limits = {
        let states = session_states.lock().await;
        states
            .get(&request.session_id.to_string())
            .and_then(|s| s.model_limits.clone())
    }
    .unwrap_or_else(|| {
        let profile_def = config
            .profiles
            .iter()
            .find(|(alias, _)| alias == &model_profile_alias)
            .map(|(_, def)| def);
        match profile_def {
            Some(def) => agent_config::resolve_limits(def),
            None => agent_models::lookup_limits("fake", "fake"), // pre-0.7 sessions
        }
    });

    let budget = crate::context_budget::build_budget(&limits);

    // Tool definitions: serialised once, consumed both by the assembler (token
    // accounting) AND by the model adapter (the actual schemas to inject).
    let tool_defs: Vec<agent_models::ToolDefinition> = {
        let registry = tool_registry.lock().await;
        registry
            .list_all()
            .await
            .into_iter()
            .map(|td| agent_models::ToolDefinition {
                name: td.tool_id,
                description: td.description,
                parameters: td.parameters,
            })
            .collect()
    };

    // Retrieve relevant memories from the MemoryStore and inject them
    // into the system prompt so the model can use prior context.
    let mut system_prompt = SYSTEM_PROMPT.to_string();
    if let Some(section) =
        crate::memory_handler::retrieve_memory_section(memory_store, &request.content).await
    {
        system_prompt.push_str(&section);
    }

    // History strings — one per narrative event. Tool-call / tool-result
    // pairing for the actual ModelMessage list happens below in
    // `build_model_messages_within_budget`; this `session_history` is purely
    // for the assembler's token accounting and dropping decisions.
    let session_history: Vec<String> = session_events
        .iter()
        .filter_map(|e| match &e.payload {
            EventPayload::UserMessageAdded { content, .. } => Some(format!("user: {content}")),
            EventPayload::AssistantMessageCompleted { content, .. } => {
                Some(format!("assistant: {content}"))
            }
            EventPayload::ToolInvocationCompleted {
                tool_id,
                output_preview,
                ..
            } => Some(format!("tool[{tool_id}]: {output_preview}")),
            _ => None,
        })
        .collect();

    let assembler = agent_memory::ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            agent_memory::ContextRequest {
                system_prompt: Some(system_prompt.clone()),
                active_skills: load_active_skill_blocks(
                    skill_registry,
                    active_skills,
                    &request.session_id,
                )
                .await?,
                user_request: request.content.clone(),
                session_history,
                tool_definitions: tool_defs.clone(),
                ..Default::default()
            },
            budget.clone(),
        )
        .await;

    // Apply per-session UsageCorrector (no-op until Task 10 wires real-usage feedback).
    let mut usage = bundle.usage.clone();
    {
        let mut states = session_states.lock().await;
        let entry = states
            .entry(request.session_id.to_string())
            .or_insert_with(crate::session::SessionState::default);
        if entry.usage_corrector.samples > 0 {
            usage.total_tokens = entry.usage_corrector.apply(usage.total_tokens);
            for (_, n) in &mut usage.by_source {
                *n = entry.usage_corrector.apply(*n);
            }
            usage.corrected_by_real_usage = true;
        }
        entry.last_estimated_tokens = usage.total_tokens;
    }

    // Emit the event so UIs can show usage.
    let assembled_event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextAssembled {
            usage: usage.clone(),
        },
    );
    append_and_broadcast(&**store, event_tx, &assembled_event).await?;

    // P2: Auto-compaction trigger. Fire-and-forget so the agent loop does
    // NOT block on the summariser LLM call. The busy gate inside
    // `compact_session` ensures we never stack two compactions for the
    // same session.
    {
        let already_compacting = {
            let states = session_states.lock().await;
            states
                .get(&request.session_id.to_string())
                .map(|s| s.compacting)
                .unwrap_or(false)
        };
        let threshold = config.context.auto_compact_threshold;
        if should_trigger_auto_compaction(&usage, threshold, already_compacting) {
            let store_clone = store.clone();
            let model_clone = model.clone();
            let tx_clone = event_tx.clone();
            let states_clone = session_states.clone();
            let workspace_id = request.workspace_id.clone();
            let session_id = request.session_id.clone();
            let ratio = usage.ratio();
            let profile_alias = config
                .context
                .compactor_profile
                .clone()
                .unwrap_or_else(|| model_profile_alias.clone());
            tokio::spawn(async move {
                let _ = crate::compaction::compact_session(
                    &*store_clone,
                    &tx_clone,
                    &*model_clone,
                    &profile_alias,
                    &states_clone,
                    workspace_id,
                    session_id,
                    agent_core::CompactionReason::Threshold { ratio },
                )
                .await;
            });
        }
    }

    // Build the actual ModelMessage list. This MUST preserve tool_call /
    // tool_result id pairing (otherwise Anthropic / OpenAI reject the request),
    // so we run the existing `build_model_messages` over `session_events` and
    // then trim the FRONT of the resulting Vec until cumulative tokens fit
    // budget.input_budget().
    let messages = build_model_messages_within_budget(
        &request.content,
        &session_events,
        budget.input_budget(),
    );

    let model_request = ModelRequest {
        model_profile: model_profile_alias,
        messages,
        system_prompt: Some(system_prompt),
        tools: tool_defs,
    };

    // Create cancellation token for this send_message call
    let cancel_token = CancellationToken::new();
    *active_cancellation.lock().await = Some(cancel_token.clone());

    // Agent loop: model -> tool call -> permission -> execute -> feed back
    let mut current_request = model_request;
    let mut iterations = 0;

    // Create root task for this message
    let root_title: String = if request.content.chars().count() > 50 {
        let truncated: String = request.content.chars().take(50).collect();
        format!("{truncated}...")
    } else {
        request.content.clone()
    };
    let root_task_id = {
        let mut task_graphs_guard = task_graphs.lock().await;
        let graph = task_graphs_guard
            .entry(request.session_id.to_string())
            .or_insert_with(TaskGraph::default);
        let root_task = graph.add_task(&root_title, AgentRole::Planner, vec![]);
        graph.mark_running(&root_task).unwrap();
        root_task
    };

    // Emit AgentTaskCreated and AgentTaskStarted for root task
    let task_created = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::AgentTaskCreated {
            task_id: root_task_id.clone(),
            title: root_title,
            role: AgentRole::Planner,
            dependencies: vec![],
        },
    );
    append_and_broadcast(&**store, event_tx, &task_created).await?;

    let task_started = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::AgentTaskStarted {
            task_id: root_task_id.clone(),
        },
    );
    append_and_broadcast(&**store, event_tx, &task_started).await?;

    loop {
        // Check if the session has been cancelled before each iteration
        if cancel_token.is_cancelled() {
            {
                let mut task_graphs_guard = task_graphs.lock().await;
                if let Some(graph) = task_graphs_guard.get_mut(&request.session_id.to_string()) {
                    let _ = graph.mark_failed(&root_task_id, "cancelled by user".into());
                }
            }
            let root_fail = DomainEvent::new(
                request.workspace_id.clone(),
                request.session_id.clone(),
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::AgentTaskFailed {
                    task_id: root_task_id.clone(),
                    error: "cancelled by user".into(),
                },
            );
            let _ = append_and_broadcast(&**store, event_tx, &root_fail).await;
            *active_cancellation.lock().await = None;
            break;
        }

        if iterations >= MAX_AGENT_LOOP_ITERATIONS {
            let event = DomainEvent::new(
                request.workspace_id.clone(),
                request.session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::AssistantMessageCompleted {
                    message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                    content: "[agent loop reached maximum iterations]".into(),
                },
            );
            append_and_broadcast(&**store, event_tx, &event).await?;

            // Mark root task as failed due to max iterations
            {
                let mut task_graphs_guard = task_graphs.lock().await;
                if let Some(graph) = task_graphs_guard.get_mut(&request.session_id.to_string()) {
                    let _ = graph.mark_failed(&root_task_id, "max iterations exceeded".into());
                }
            }
            let root_fail = DomainEvent::new(
                request.workspace_id.clone(),
                request.session_id.clone(),
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::AgentTaskFailed {
                    task_id: root_task_id.clone(),
                    error: "max iterations exceeded".into(),
                },
            );
            let _ = append_and_broadcast(&**store, event_tx, &root_fail).await;
            *active_cancellation.lock().await = None;

            break;
        }
        iterations += 1;

        let stream_result = model.stream(current_request.clone()).await;

        let mut stream = match stream_result {
            Ok(s) => s,
            Err(e) => {
                let error_msg = e.to_string();
                let fail_event = DomainEvent::new(
                    request.workspace_id.clone(),
                    request.session_id.clone(),
                    AgentId::system(),
                    PrivacyClassification::FullTrace,
                    EventPayload::AgentTaskFailed {
                        task_id: TaskId::new(),
                        error: error_msg.clone(),
                    },
                );
                let _ = append_and_broadcast(&**store, event_tx, &fail_event).await;
                // Mark root task as failed
                {
                    let mut task_graphs_guard = task_graphs.lock().await;
                    if let Some(graph) = task_graphs_guard.get_mut(&request.session_id.to_string())
                    {
                        let _ = graph.mark_failed(&root_task_id, error_msg.clone());
                    }
                }
                let root_fail = DomainEvent::new(
                    request.workspace_id.clone(),
                    request.session_id.clone(),
                    AgentId::system(),
                    PrivacyClassification::MinimalTrace,
                    EventPayload::AgentTaskFailed {
                        task_id: root_task_id.clone(),
                        error: error_msg.clone(),
                    },
                );
                let _ = append_and_broadcast(&**store, event_tx, &root_fail).await;
                *active_cancellation.lock().await = None;
                return Err(agent_core::CoreError::InvalidState(error_msg));
            }
        };

        let mut assistant_text = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(ModelEvent::TokenDelta(delta)) => {
                    assistant_text.push_str(&delta);
                    let event = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::ModelTokenDelta { delta },
                    );
                    append_and_broadcast(&**store, event_tx, &event).await?;
                    if cancel_token.is_cancelled() {
                        break;
                    }
                }
                Ok(ModelEvent::ToolCallRequested {
                    tool_call_id,
                    tool_id,
                    arguments,
                }) => {
                    let event = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::ModelToolCallRequested {
                            tool_call_id: tool_call_id.clone(),
                            tool_id: tool_id.clone(),
                        },
                    );
                    append_and_broadcast(&**store, event_tx, &event).await?;
                    tool_calls.push(ToolCall {
                        id: tool_call_id,
                        name: tool_id,
                        arguments,
                    });
                }
                Ok(ModelEvent::Completed { usage: real_usage }) => {
                    // Feed real input-token usage back into the per-session
                    // UsageCorrector so the next iteration's cl100k_base
                    // estimate is multiplied by an EMA-smoothed correction
                    // factor (clamped to [0.7, 1.5]). Anthropic + OpenAI
                    // populate `usage`; Ollama leaves it None today.
                    if let Some(u) = real_usage {
                        let mut states = deps.session_states.lock().await;
                        if let Some(entry) = states.get_mut(request.session_id.as_str()) {
                            let estimated = entry.last_estimated_tokens;
                            if estimated > 0 {
                                entry.usage_corrector.update(u.input_tokens, estimated);
                            }
                        }
                    }
                    // Always emit AssistantMessageCompleted when the model
                    // finishes, even with empty text (e.g., tool-only response).
                    // The GUI relies on this event to reset the streaming state.
                    let display_content = if assistant_text.is_empty() {
                        String::new()
                    } else {
                        strip_memory_markers(&assistant_text)
                    };
                    let event = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::AssistantMessageCompleted {
                            message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                            content: display_content,
                        },
                    );
                    append_and_broadcast(&**store, event_tx, &event).await?;
                }
                Ok(ModelEvent::Failed { message }) => {
                    let fail_event = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::AgentTaskFailed {
                            task_id: TaskId::new(),
                            error: message.clone(),
                        },
                    );
                    let _ = append_and_broadcast(&**store, event_tx, &fail_event).await;
                    // Mark root task as failed
                    {
                        let mut task_graphs_guard = task_graphs.lock().await;
                        if let Some(graph) =
                            task_graphs_guard.get_mut(&request.session_id.to_string())
                        {
                            let _ = graph.mark_failed(&root_task_id, message.clone());
                        }
                    }
                    let root_fail = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::MinimalTrace,
                        EventPayload::AgentTaskFailed {
                            task_id: root_task_id.clone(),
                            error: message.clone(),
                        },
                    );
                    let _ = append_and_broadcast(&**store, event_tx, &root_fail).await;
                    *active_cancellation.lock().await = None;
                    return Err(agent_core::CoreError::InvalidState(message));
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    let fail_event = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::AgentTaskFailed {
                            task_id: TaskId::new(),
                            error: error_msg.clone(),
                        },
                    );
                    let _ = append_and_broadcast(&**store, event_tx, &fail_event).await;
                    // Mark root task as failed
                    {
                        let mut task_graphs_guard = task_graphs.lock().await;
                        if let Some(graph) =
                            task_graphs_guard.get_mut(&request.session_id.to_string())
                        {
                            let _ = graph.mark_failed(&root_task_id, error_msg.clone());
                        }
                    }
                    let root_fail = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::MinimalTrace,
                        EventPayload::AgentTaskFailed {
                            task_id: root_task_id.clone(),
                            error: error_msg.clone(),
                        },
                    );
                    let _ = append_and_broadcast(&**store, event_tx, &root_fail).await;
                    *active_cancellation.lock().await = None;
                    return Err(agent_core::CoreError::InvalidState(error_msg));
                }
            }
        }

        // Process memory markers from assistant response
        crate::memory_handler::store_memory_markers(
            &**store,
            event_tx,
            permission_engine,
            pending_permissions,
            memory_store,
            &request.workspace_id,
            &request.session_id,
            &assistant_text,
        )
        .await;

        // If no tool calls, the agent loop ends — mark root task as completed
        if tool_calls.is_empty() {
            {
                let mut task_graphs_guard = task_graphs.lock().await;
                if let Some(graph) = task_graphs_guard.get_mut(&request.session_id.to_string()) {
                    let _ = graph.mark_completed(&root_task_id);
                }
            }
            let root_done = DomainEvent::new(
                request.workspace_id.clone(),
                request.session_id.clone(),
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::AgentTaskCompleted {
                    task_id: root_task_id.clone(),
                },
            );
            let _ = append_and_broadcast(&**store, event_tx, &root_done).await;
            break;
        }

        // Process tool calls through permission and execution
        let registry = tool_registry.lock().await;
        for tc in &tool_calls {
            // Check permission
            let risk = if let Some(tool) = registry.get(&tc.name).await {
                let inv = ToolInvocation {
                    tool_id: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                    workspace_id: request.workspace_id.to_string(),
                    preview: format!("{}({})", tc.name, tc.arguments),
                    timeout_ms: 30_000,
                    output_limit_bytes: 102_400,
                };
                tool.risk(&inv)
            } else {
                agent_tools::ToolRisk::read(&tc.name)
            };

            let preview = format!("{}({})", tc.name, tc.arguments);
            let perm_result = crate::permission::check_tool_permission(
                &**store,
                event_tx,
                permission_engine,
                pending_permissions,
                &request.workspace_id,
                &request.session_id,
                &tc.id,
                &tc.name,
                &preview,
                &risk,
            )
            .await?;
            let permission_event = perm_result.event;
            let should_execute = perm_result.should_execute;
            append_and_broadcast(&**store, event_tx, &permission_event).await?;

            if should_execute {
                // Create sub-task for this tool call
                let sub_task_id = {
                    let mut task_graphs_guard = task_graphs.lock().await;
                    if let Some(graph) = task_graphs_guard.get_mut(&request.session_id.to_string())
                    {
                        let sub_task =
                            graph.add_task(&tc.name, AgentRole::Worker, vec![root_task_id.clone()]);
                        graph.mark_running(&sub_task).unwrap();
                        Some(sub_task)
                    } else {
                        None
                    }
                };

                if let Some(ref sub_id) = sub_task_id {
                    let sub_created = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::MinimalTrace,
                        EventPayload::AgentTaskCreated {
                            task_id: sub_id.clone(),
                            title: tc.name.clone(),
                            role: AgentRole::Worker,
                            dependencies: vec![root_task_id.clone()],
                        },
                    );
                    let _ = append_and_broadcast(&**store, event_tx, &sub_created).await;

                    let sub_started = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::MinimalTrace,
                        EventPayload::AgentTaskStarted {
                            task_id: sub_id.clone(),
                        },
                    );
                    let _ = append_and_broadcast(&**store, event_tx, &sub_started).await;
                }

                let invocation = ToolInvocation {
                    tool_id: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                    workspace_id: request.workspace_id.to_string(),
                    preview: format!("{}({})", tc.name, tc.arguments),
                    timeout_ms: 30_000,
                    output_limit_bytes: 102_400,
                };

                let tool_start = std::time::Instant::now();

                let start_event = DomainEvent::new(
                    request.workspace_id.clone(),
                    request.session_id.clone(),
                    AgentId::system(),
                    PrivacyClassification::FullTrace,
                    EventPayload::ToolInvocationStarted {
                        invocation_id: tc.id.clone(),
                        tool_id: tc.name.clone(),
                    },
                );
                append_and_broadcast(&**store, event_tx, &start_event).await?;

                let result = registry
                    .invoke_with_permission(&*permission_engine.lock().await, invocation)
                    .await;

                let completion_event = match result {
                    Ok(output) => DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::ToolInvocationCompleted {
                            invocation_id: tc.id.clone(),
                            tool_id: tc.name.clone(),
                            output_preview: output.text.chars().take(500).collect(),
                            exit_code: None,
                            duration_ms: tool_start.elapsed().as_millis() as u64,
                            truncated: output.truncated,
                        },
                    ),
                    Err(e) => DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::ToolInvocationFailed {
                            invocation_id: tc.id.clone(),
                            tool_id: tc.name.clone(),
                            error: e.to_string(),
                        },
                    ),
                };
                append_and_broadcast(&**store, event_tx, &completion_event).await?;

                // Mark sub-task as completed or failed
                if let Some(sub_id) = sub_task_id {
                    let task_event = match &completion_event.payload {
                        EventPayload::ToolInvocationCompleted { .. } => {
                            {
                                let mut task_graphs_guard = task_graphs.lock().await;
                                if let Some(graph) =
                                    task_graphs_guard.get_mut(&request.session_id.to_string())
                                {
                                    let _ = graph.mark_completed(&sub_id);
                                }
                            }
                            Some(DomainEvent::new(
                                request.workspace_id.clone(),
                                request.session_id.clone(),
                                AgentId::system(),
                                PrivacyClassification::MinimalTrace,
                                EventPayload::AgentTaskCompleted { task_id: sub_id },
                            ))
                        }
                        EventPayload::ToolInvocationFailed { error, .. } => {
                            {
                                let mut task_graphs_guard = task_graphs.lock().await;
                                if let Some(graph) =
                                    task_graphs_guard.get_mut(&request.session_id.to_string())
                                {
                                    let _ = graph.mark_failed(&sub_id, error.clone());
                                }
                            }
                            Some(DomainEvent::new(
                                request.workspace_id.clone(),
                                request.session_id.clone(),
                                AgentId::system(),
                                PrivacyClassification::MinimalTrace,
                                EventPayload::AgentTaskFailed {
                                    task_id: sub_id,
                                    error: error.clone(),
                                },
                            ))
                        }
                        _ => None,
                    };
                    if let Some(evt) = task_event {
                        let _ = append_and_broadcast(&**store, event_tx, &evt).await;
                    }
                }
            }
        }
        drop(registry);

        // Build next request with tool results appended.
        // For tool calls where permission was denied (no ToolInvocationCompleted
        // event exists), we still need to include a tool result so the model
        // knows the tool was not executed and can respond accordingly.
        //
        // IMPORTANT: We include the tool_calls in the assistant message so that
        // model adapters (Anthropic, OpenAI) can generate the required
        // tool_use/tool_calls blocks in the API request format. Without this,
        // the Anthropic API rejects requests where tool_result follows an
        // assistant message without tool_use blocks.
        let tool_calls_for_msg: Vec<agent_models::ToolCall> = tool_calls
            .iter()
            .map(|tc| agent_models::ToolCall {
                id: tc.id.clone(),
                name: tc.name.clone(),
                arguments: tc.arguments.clone(),
            })
            .collect();
        current_request = current_request
            .clone()
            .add_assistant_with_tools(&assistant_text, tool_calls_for_msg);
        let session_events = store
            .load_session(&request.session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        for tc in &tool_calls {
            let tool_results_for_call: Vec<String> = session_events
                .iter()
                .filter_map(|e| match &e.payload {
                    EventPayload::ToolInvocationCompleted {
                        invocation_id,
                        output_preview,
                        ..
                    } => {
                        if invocation_id == &tc.id {
                            Some(output_preview.clone())
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .collect();
            // Use add_tool_result so that model adapters can map the
            // result back to the correct tool call via tool_call_id.
            // This is required by Anthropic (tool_use_id) and OpenAI (tool_call_id).
            if !tool_results_for_call.is_empty() {
                let result_content = format!(
                    "tool_id={}\nresult={}",
                    tc.name,
                    tool_results_for_call.join("\n")
                );
                current_request = current_request.add_tool_result(&tc.id, &result_content);
            } else {
                // No ToolInvocationCompleted for this call - permission was denied
                // or the invocation failed. Provide a fallback result so the
                // model knows the tool was not executed.
                let permission_denied = session_events.iter().any(|e| {
                    matches!(
                        &e.payload,
                        EventPayload::PermissionDenied { request_id, .. }
                        if request_id == &tc.id
                    )
                });
                let denial_reason = if permission_denied {
                    "Permission denied by user"
                } else {
                    "Tool invocation failed or was not executed"
                };
                current_request = current_request.add_tool_result(
                    &tc.id,
                    format!("tool_id={}\nresult=Error: {}", tc.name, denial_reason),
                );
            }
        }
    }

    // Clean up cancellation token on normal completion
    *active_cancellation.lock().await = None;

    Ok(())
}

/// Decide whether the agent loop should fire an auto-compaction request
/// for this iteration. Pure function so it's trivial to unit-test the
/// boundary cases (threshold == 1.0 disables; busy gate skips; exact
/// equality counts as crossing the threshold per spec §4.4).
pub fn should_trigger_auto_compaction(
    usage: &agent_core::ContextUsage,
    threshold: f32,
    already_compacting: bool,
) -> bool {
    if already_compacting || threshold >= 1.0 {
        return false;
    }
    usage.ratio() >= threshold
}

/// Builds a `Vec<ModelMessage>` from `session_events` (preserving tool_call /
/// tool_result id pairing) and trims the FRONT until cumulative input tokens
/// fit `budget_tokens`. The system prompt + the most-recent user message are
/// always kept (they're appended last by `build_model_messages`).
///
/// Token accounting MUST match what providers actually bill — `ModelMessage`
/// has three serialised parts: `role`, `content`, and `tool_calls`
/// (a `Vec<ToolCall>` whose `arguments` is `serde_json::Value`). Tool calls
/// alone often weigh thousands of tokens for non-trivial payloads, so we
/// serialise the whole message to JSON and count that. This matches the
/// estimator used by `ContextAssembler` (cl100k_base on serialised text).
pub fn build_model_messages_within_budget(
    user_content: &str,
    session_events: &[DomainEvent],
    budget_tokens: u64,
) -> Vec<agent_models::ModelMessage> {
    let mut messages = build_model_messages(user_content, session_events);

    let bpe = match tiktoken_rs::cl100k_base() {
        Ok(bpe) => bpe,
        Err(_) => return messages, // tokenizer unavailable; emit as-is
    };
    let count_message = |m: &agent_models::ModelMessage| -> u64 {
        // Use compact JSON to mirror what the OpenAI/Anthropic adapters
        // ultimately serialise. Failures fall back to content-only count.
        match serde_json::to_string(m) {
            Ok(s) => bpe.encode_with_special_tokens(&s).len() as u64,
            Err(_) => bpe.encode_with_special_tokens(&m.content).len() as u64,
        }
    };

    // Always keep the trailing user message (the active turn). Trim from the
    // FRONT, but NEVER drop a `tool` role message without also dropping the
    // matching assistant `tool_calls` message that precedes it — otherwise
    // OpenAI / Anthropic reject the request with "tool_call_id has no
    // matching assistant tool_calls".
    let mut total: u64 = messages.iter().map(&count_message).sum();
    while total > budget_tokens && messages.len() > 1 {
        let front = messages.first().unwrap();
        if front.role == "tool" {
            // No matching assistant left at the front — safe to drop alone.
            total = total.saturating_sub(count_message(front));
            messages.remove(0);
            continue;
        }
        if front.role == "assistant" && !front.tool_calls.is_empty() {
            // Drop the assistant AND every tool message immediately following it
            // (the matching `tool_call_id` results) in one atomic step.
            total = total.saturating_sub(count_message(front));
            messages.remove(0);
            while !messages.is_empty() && messages[0].role == "tool" {
                total = total.saturating_sub(count_message(&messages[0]));
                messages.remove(0);
            }
            continue;
        }
        // Plain user/assistant text — drop one.
        total = total.saturating_sub(count_message(front));
        messages.remove(0);
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AgentId, PrivacyClassification, SessionId, WorkspaceId};

    fn make_event(payload: EventPayload) -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
    }

    #[test]
    fn build_model_messages_substitutes_compaction_summary_for_event_range() {
        // Build 5 turns; insert a CompactionSummary covering the first 3 pairs.
        let base = chrono::Utc::now();
        let make_at = |payload: EventPayload, secs: i64| -> DomainEvent {
            DomainEvent::new(
                WorkspaceId::new(),
                SessionId::new(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                payload,
            )
            .with_timestamp(base + chrono::Duration::seconds(secs))
        };

        let mut events: Vec<DomainEvent> = (0..5)
            .flat_map(|i| {
                let t = (i as i64) * 10;
                vec![
                    make_at(
                        EventPayload::UserMessageAdded {
                            message_id: format!("u{i}"),
                            content: format!("user {i}"),
                        },
                        t,
                    ),
                    make_at(
                        EventPayload::AssistantMessageCompleted {
                            message_id: format!("a{i}"),
                            content: format!("assistant {i}"),
                        },
                        t + 1,
                    ),
                ]
            })
            .collect();

        let first_ts = events[0].timestamp;
        let last_ts = events[5].timestamp; // covers pairs 0..=2 inclusive
        events.push(make_at(
            EventPayload::CompactionSummary {
                summary_id: "sum_test".into(),
                content: "[SUMMARY] earlier turns about user goal X".into(),
                replaces_event_range: (first_ts, last_ts),
                reason: agent_core::CompactionReason::UserRequested,
                before_tokens: 1000,
                after_tokens: 50,
                summarised_by_profile: "fast".into(),
            },
            55, // newer than every replaced event but older than the new turn
        ));
        events.sort_by_key(|e| e.timestamp);

        let messages = build_model_messages("latest", &events);

        // (a) The summary text MUST appear in messages.
        let joined: String = messages
            .iter()
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("[SUMMARY] earlier turns about user goal X"),
            "summary text missing from assembled messages: {joined}"
        );
        // (b) The replaced "user 0".."assistant 2" content must NOT appear.
        for replaced in [
            "user 0",
            "assistant 0",
            "user 1",
            "assistant 1",
            "user 2",
            "assistant 2",
        ] {
            assert!(
                !joined.contains(replaced),
                "replaced event '{replaced}' leaked into messages: {joined}"
            );
        }
        // (c) The kept tail ("user 3", "assistant 3", "user 4", "assistant 4") must remain.
        for kept in ["user 3", "assistant 3", "user 4", "assistant 4"] {
            assert!(
                joined.contains(kept),
                "kept event '{kept}' missing from messages: {joined}"
            );
        }
        // (d) The trailing "latest" user turn must still be present.
        assert_eq!(messages.last().map(|m| m.content.as_str()), Some("latest"));
    }

    #[test]
    fn should_trigger_auto_compaction_uses_threshold_and_not_compacting() {
        let usage_at = |total: u64, budget: u64| -> agent_core::ContextUsage {
            agent_core::ContextUsage {
                total_tokens: total,
                budget_tokens: budget,
                context_window: budget + 12_000,
                output_reservation: 12_000,
                by_source: vec![],
                estimator: "cl100k_base".into(),
                corrected_by_real_usage: false,
            }
        };

        // Below threshold → no trigger.
        assert!(!should_trigger_auto_compaction(
            &usage_at(50_000, 200_000),
            0.85,
            false
        ));
        // At threshold → trigger.
        assert!(should_trigger_auto_compaction(
            &usage_at(170_000, 200_000),
            0.85,
            false
        ));
        // Above threshold but already compacting → no trigger.
        assert!(!should_trigger_auto_compaction(
            &usage_at(190_000, 200_000),
            0.85,
            true
        ));
        // Threshold == 1.0 disables auto-compaction entirely.
        assert!(!should_trigger_auto_compaction(
            &usage_at(199_000, 200_000),
            1.0,
            false
        ));
    }

    #[test]
    fn within_budget_keeps_tail_user_and_pairs_tool_calls() {
        // Build 3 plain user/assistant pairs, each padded so cumulative tokens
        // exceed the 100-token budget and the trimmer must drop from the front.
        let mut events = Vec::new();
        for i in 0..3 {
            events.push(make_event(EventPayload::UserMessageAdded {
                message_id: format!("u{i}"),
                content: format!("user turn {i} ").repeat(20),
            }));
            events.push(make_event(EventPayload::AssistantMessageCompleted {
                message_id: format!("a{i}"),
                content: format!("assistant turn {i} ").repeat(20),
            }));
        }

        let trimmed = build_model_messages_within_budget("latest", &events, 100);

        // (a) total token count <= 100
        let bpe = tiktoken_rs::cl100k_base().unwrap();
        let total: usize = trimmed
            .iter()
            .map(|m| {
                bpe.encode_with_special_tokens(&serde_json::to_string(m).unwrap())
                    .len()
            })
            .sum();
        assert!(total <= 100, "trimmed total {} exceeded budget 100", total);

        // (b) trailing user message is the active turn
        assert_eq!(trimmed.last().map(|m| m.role.as_str()), Some("user"));
        assert_eq!(trimmed.last().map(|m| m.content.as_str()), Some("latest"));

        // (c) every `tool` role message has a preceding assistant with non-empty tool_calls
        for (i, m) in trimmed.iter().enumerate() {
            if m.role == "tool" {
                assert!(i > 0, "tool message at index 0 is unpaired");
                let prev = &trimmed[i - 1];
                assert!(
                    prev.role == "assistant" && !prev.tool_calls.is_empty(),
                    "tool message at {} not preceded by assistant with tool_calls",
                    i
                );
            }
        }
    }
}

#[cfg(test)]
mod model_profile_resolution_tests {
    use super::latest_model_profile_for;
    use agent_core::{
        AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
    };

    fn init_event(profile: &str) -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::SessionInitialized {
                model_profile: profile.into(),
            },
        )
    }

    fn switch_event(from: &str, to: &str) -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::ModelProfileSwitched {
                from_profile: from.into(),
                to_profile: to.into(),
                effective_at: chrono::Utc::now(),
                context_window: 0,
                output_limit: 0,
                limit_source: "fallback".into(),
            },
        )
    }

    #[test]
    fn returns_session_initialized_profile_when_no_switch() {
        let events = vec![init_event("fast")];
        assert_eq!(latest_model_profile_for(&events), "fast");
    }

    #[test]
    fn returns_latest_switch_when_one_exists() {
        let events = vec![init_event("fast"), switch_event("fast", "claude-opus")];
        assert_eq!(latest_model_profile_for(&events), "claude-opus");
    }

    #[test]
    fn returns_most_recent_switch_when_multiple_exist() {
        let events = vec![
            init_event("fast"),
            switch_event("fast", "gpt-4o"),
            switch_event("gpt-4o", "claude-opus"),
        ];
        assert_eq!(latest_model_profile_for(&events), "claude-opus");
    }

    #[test]
    fn falls_back_to_fake_when_no_initialization_event() {
        let events: Vec<DomainEvent> = vec![];
        assert_eq!(latest_model_profile_for(&events), "fake");
    }
}

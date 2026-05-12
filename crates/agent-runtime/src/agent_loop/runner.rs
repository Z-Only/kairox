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
use agent_tools::{PermissionEngine, ToolRegistry};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

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
    let mut system_prompt = super::SYSTEM_PROMPT.to_string();
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

    let active_skill_blocks =
        load_active_skill_blocks(skill_registry, active_skills, &request.session_id).await?;

    let assembler = agent_memory::ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            agent_memory::ContextRequest {
                system_prompt: Some(system_prompt.clone()),
                active_skills: active_skill_blocks.clone(),
                user_request: request.content.clone(),
                session_history,
                tool_definitions: tool_defs.clone(),
                ..Default::default()
            },
            budget.clone(),
        )
        .await;

    if !active_skill_blocks.is_empty() {
        system_prompt.push_str("\n\n<active_skills>\n");
        system_prompt.push_str(&active_skill_blocks.join("\n"));
        system_prompt.push_str("\n</active_skills>");
    }

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
        if super::should_trigger_auto_compaction(&usage, threshold, already_compacting) {
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
    let messages = super::build_model_messages_within_budget(
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

        if iterations >= super::MAX_AGENT_LOOP_ITERATIONS {
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

        // Execute all tool calls (permission, sub-tasks, invocation, events).
        let tool_loop_result = super::execute_tool_calls(
            &tool_calls,
            tool_registry,
            permission_engine,
            store,
            event_tx,
            &request.workspace_id,
            &request.session_id,
            pending_permissions,
            task_graphs,
            &root_task_id,
        )
        .await?;

        // Build next request with tool results appended.
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
        for (tool_call_id, output_text) in &tool_loop_result.tool_results {
            current_request = current_request.add_tool_result(tool_call_id, output_text);
        }
    }

    // Clean up cancellation token on normal completion
    *active_cancellation.lock().await = None;

    Ok(())
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

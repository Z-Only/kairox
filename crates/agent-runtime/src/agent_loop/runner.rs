use crate::agent_loop::{
    prepare_turn_context, process_model_stream, AgentLoopDeps, StreamOutput, TurnContext,
};
use crate::event_emitter::append_and_broadcast;
use crate::skills::render_active_skill_block;
use crate::task_graph::TaskGraph;
use agent_core::{
    AgentId, AgentRole, DomainEvent, EventPayload, PrivacyClassification, SendMessageRequest,
    TaskId, TrajectoryId,
};
use agent_models::{ModelClient, ModelRequest};
use agent_store::{EventStore, TrajectoryStore};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Resolve the active model profile alias from a session's full event log.
///
/// Priority (newest to oldest wins):
/// 1. The most recent `EventPayload::ModelProfileSwitched.to_profile`.
/// 2. `EventPayload::SessionInitialized.model_profile` (the session's
///    original profile).
/// 3. The literal `"fake"` (only reached for broken event logs).
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

pub(crate) fn latest_model_reasoning_effort_for(
    events: &[agent_core::DomainEvent],
) -> Option<String> {
    for event in events.iter().rev() {
        if let agent_core::EventPayload::ModelProfileSwitched {
            reasoning_effort, ..
        } = &event.payload
        {
            return reasoning_effort.clone();
        }
    }
    None
}

pub(crate) async fn load_active_skill_blocks(
    skill_registry: &Option<Arc<dyn agent_skills::SkillRegistry>>,
    active_skills: &Arc<Mutex<HashMap<String, Vec<String>>>>,
    session_id: &agent_core::SessionId,
    session_events: &[agent_core::DomainEvent],
) -> agent_core::Result<Vec<String>> {
    let Some(registry) = skill_registry else {
        return Ok(Vec::new());
    };
    let session_key = session_id.to_string();
    let skill_ids = {
        let mut active_skills = active_skills.lock().await;
        let mut skill_ids = active_skills
            .get(&session_key)
            .cloned()
            .unwrap_or_else(|| crate::skills::active_skill_ids_from_events(session_events));
        skill_ids.retain(|skill_id| {
            registry
                .get(&agent_skills::SkillId::new(skill_id.clone()))
                .is_some()
        });
        active_skills.insert(session_key.clone(), skill_ids.clone());
        skill_ids
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

pub(crate) async fn run_agent_loop<S, M>(
    deps: AgentLoopDeps<'_, S, M>,
    request: &SendMessageRequest,
) -> agent_core::Result<()>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    let display_content = request
        .display_content
        .as_ref()
        .filter(|content| content.as_str() != request.content)
        .cloned();
    let user_display_content = display_content
        .clone()
        .unwrap_or_else(|| request.content.clone());

    // ── 1. Record user message ──────────────────────────────────────
    let user_event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::UserMessageAdded {
            message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
            content: request.content.clone(),
            display_content,
        },
    );
    append_and_broadcast(&**deps.store, deps.event_tx, &user_event).await?;

    run_lifecycle_hooks(
        deps.config,
        agent_config::HookEvent::UserPromptSubmit,
        "*",
        deps.root_path.as_deref(),
        serde_json::json!({
            "workspace_id": request.workspace_id.as_str(),
            "session_id": request.session_id.as_str(),
            "content": user_display_content.as_str(),
            "model_content": request.content.as_str(),
        }),
    )
    .await;

    // ── 2. Load session events ──────────────────────────────────────
    let session_events = deps
        .store
        .load_session(&request.session_id)
        .await
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

    // ── 3. Prepare turn context ─────────────────────────────────────
    let TurnContext {
        model_profile_alias,
        reasoning_effort,
        budget,
        system_prompt,
        tool_definitions,
        server_tools,
    } = prepare_turn_context(&deps, request, &session_events).await?;

    // ── 4. Build model messages + request ───────────────────────────
    let messages = super::build_model_messages_within_budget(
        &request.content,
        &session_events,
        budget.input_budget(),
    );

    let model_request = ModelRequest {
        model_profile: model_profile_alias,
        messages,
        system_prompt: Some(system_prompt),
        tools: tool_definitions,
        server_tools,
        reasoning_effort,
    };

    // ── 5. Cancellation token ───────────────────────────────────────
    let cancel_token = deps.turn_cancellation.clone();

    // ── 6. Create root task ─────────────────────────────────────────
    let root_title: String = if user_display_content.chars().count() > 50 {
        let truncated: String = user_display_content.chars().take(50).collect();
        format!("{truncated}...")
    } else {
        user_display_content.clone()
    };
    let root_task_id = {
        let mut guard = deps.task_graphs.lock().await;
        let graph = guard
            .entry(request.session_id.to_string())
            .or_insert_with(TaskGraph::default);
        let root = graph.add_task(&root_title, AgentRole::Planner, vec![]);
        graph
            .mark_running(&root)
            .expect("root task was just added to graph");
        root
    };
    emit_task_created_and_started(
        &**deps.store,
        deps.event_tx,
        request,
        &root_task_id,
        &root_title,
        AgentRole::Planner,
        &[],
    )
    .await?;

    // ── 6b. Start trajectory recording ─────────────────────────────
    let trajectory_id = if deps.trajectory_store.is_some() {
        let id = TrajectoryId::new();
        if let Some(ts) = deps.trajectory_store.as_ref() {
            if let Err(e) = ts
                .start_trajectory(&id, root_task_id.as_str(), request.session_id.as_str())
                .await
            {
                tracing::warn!("failed to start trajectory: {e}");
            } else {
                let event = DomainEvent::new(
                    request.workspace_id.clone(),
                    request.session_id.clone(),
                    AgentId::system(),
                    PrivacyClassification::FullTrace,
                    EventPayload::TrajectoryStarted {
                        trajectory_id: id.to_string(),
                        task_id: root_task_id.to_string(),
                    },
                );
                let _ = append_and_broadcast(&**deps.store, deps.event_tx, &event).await;
            }
        }
        Some(id)
    } else {
        None
    };
    let trajectory_step_counter = std::sync::atomic::AtomicU32::new(0);

    // ── 7. Agent loop ───────────────────────────────────────────────
    let mut current_request = model_request;
    let mut iterations = 0;

    loop {
        // Guard: cancellation
        if cancel_token.is_cancelled() {
            cancel_root_task(
                &**deps.store,
                deps.event_tx,
                deps.task_graphs,
                request,
                &root_task_id,
            )
            .await;
            complete_trajectory(
                deps.trajectory_store,
                deps.store,
                deps.event_tx,
                request,
                &trajectory_id,
                &trajectory_step_counter,
                agent_core::TrajectoryOutcome::Cancelled,
            )
            .await;
            break;
        }

        // Guard: max iterations
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
            append_and_broadcast(&**deps.store, deps.event_tx, &event).await?;
            fail_root_task(
                &**deps.store,
                deps.event_tx,
                deps.task_graphs,
                request,
                &root_task_id,
                "max iterations exceeded",
            )
            .await;
            complete_trajectory(
                deps.trajectory_store,
                deps.store,
                deps.event_tx,
                request,
                &trajectory_id,
                &trajectory_step_counter,
                agent_core::TrajectoryOutcome::Failed,
            )
            .await;
            break;
        }
        iterations += 1;

        // Stream from model → handle events
        let StreamOutput {
            assistant_text,
            tool_calls,
        } = process_model_stream(
            &deps,
            request,
            &cancel_token,
            &root_task_id,
            &current_request,
        )
        .await?;

        if cancel_token.is_cancelled() {
            cancel_root_task(
                &**deps.store,
                deps.event_tx,
                deps.task_graphs,
                request,
                &root_task_id,
            )
            .await;
            complete_trajectory(
                deps.trajectory_store,
                deps.store,
                deps.event_tx,
                request,
                &trajectory_id,
                &trajectory_step_counter,
                agent_core::TrajectoryOutcome::Cancelled,
            )
            .await;
            break;
        }

        // Process memory markers
        crate::memory_handler::store_memory_markers(
            &**deps.store,
            deps.event_tx,
            deps.memory_store,
            &request.workspace_id,
            &request.session_id,
            &assistant_text,
        )
        .await;

        // No tool calls → turn complete
        if tool_calls.is_empty() {
            complete_root_task(
                &**deps.store,
                deps.event_tx,
                deps.task_graphs,
                request,
                &root_task_id,
            )
            .await;
            complete_trajectory(
                deps.trajectory_store,
                deps.store,
                deps.event_tx,
                request,
                &trajectory_id,
                &trajectory_step_counter,
                agent_core::TrajectoryOutcome::Success,
            )
            .await;
            run_lifecycle_hooks(
                deps.config,
                agent_config::HookEvent::Stop,
                "complete",
                deps.root_path.as_deref(),
                serde_json::json!({
                    "workspace_id": request.workspace_id.as_str(),
                    "session_id": request.session_id.as_str(),
                    "reason": "complete",
                }),
            )
            .await;
            break;
        }

        // ── Advisor review (optional) ───────────────────────────────
        let advisor_config = &deps.config.advisor;
        if crate::advisor::should_review(advisor_config.mode, &tool_calls) {
            let advisor_profile = advisor_config
                .profile
                .clone()
                .unwrap_or_else(|| current_request.model_profile.clone());

            let review_id = format!("adv_{}", uuid::Uuid::new_v4().simple());

            // Emit AdvisorReviewStarted
            let started_event = DomainEvent::new(
                request.workspace_id.clone(),
                request.session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::AdvisorReviewStarted {
                    review_id: review_id.clone(),
                    advisor_profile: advisor_profile.clone(),
                    tool_call_count: tool_calls.len(),
                    mode: advisor_config.mode,
                },
            );
            let _ = append_and_broadcast(&**deps.store, deps.event_tx, &started_event).await;

            if let Some(review) = crate::advisor::review_tool_calls(
                &**deps.model,
                &advisor_profile,
                &tool_calls,
                &assistant_text,
                advisor_config.max_concerns,
            )
            .await
            {
                // Emit AdvisorReviewCompleted
                let completed_event = DomainEvent::new(
                    request.workspace_id.clone(),
                    request.session_id.clone(),
                    AgentId::system(),
                    PrivacyClassification::FullTrace,
                    EventPayload::AdvisorReviewCompleted {
                        review_id: review_id.clone(),
                        verdict: review.verdict.clone(),
                        concern_count: review.concerns.len(),
                        summary: review.summary.clone(),
                    },
                );
                let _ = append_and_broadcast(&**deps.store, deps.event_tx, &completed_event).await;

                // If the advisor rejects, emit a message and skip tool execution
                if review.verdict == agent_core::AdvisorVerdict::Reject {
                    let rejection_msg = format!(
                        "[Advisor blocked tool execution]\n\n**Verdict**: Rejected\n**Summary**: {}\n\n**Concerns**:\n{}",
                        review.summary,
                        review.concerns.iter().map(|c| format!("- [{}] {}: {}", c.severity, c.tool_name, c.message)).collect::<Vec<_>>().join("\n")
                    );
                    let event = DomainEvent::new(
                        request.workspace_id.clone(),
                        request.session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::AssistantMessageCompleted {
                            message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                            content: rejection_msg,
                        },
                    );
                    append_and_broadcast(&**deps.store, deps.event_tx, &event).await?;

                    complete_root_task(
                        &**deps.store,
                        deps.event_tx,
                        deps.task_graphs,
                        request,
                        &root_task_id,
                    )
                    .await;
                    complete_trajectory(
                        deps.trajectory_store,
                        deps.store,
                        deps.event_tx,
                        request,
                        &trajectory_id,
                        &trajectory_step_counter,
                        agent_core::TrajectoryOutcome::Failed,
                    )
                    .await;
                    break;
                }
            }
        }

        // Execute tool calls
        let tool_loop_result = super::execute_tool_calls(
            &tool_calls,
            deps.tool_registry,
            deps.permission_engine,
            deps.store,
            deps.event_tx,
            &request.workspace_id,
            &request.session_id,
            deps.pending_permissions,
            deps.task_graphs,
            &root_task_id,
            deps.config,
            deps.workspace_scoped_builtin_tools,
            deps.root_path.as_deref(),
            &cancel_token,
            deps.trajectory_store,
            &trajectory_id,
            &trajectory_step_counter,
        )
        .await?;

        // Build next request with tool results appended
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

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────

async fn run_lifecycle_hooks(
    config: &agent_config::Config,
    event: agent_config::HookEvent,
    matcher_value: &str,
    root_path: Option<&std::path::Path>,
    payload: serde_json::Value,
) {
    crate::hooks::run_hooks_logged(config, event, matcher_value, root_path, payload).await;
}

async fn emit_task_created_and_started<S: EventStore + 'static>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    request: &SendMessageRequest,
    task_id: &TaskId,
    title: &str,
    role: AgentRole,
    dependencies: &[TaskId],
) -> agent_core::Result<()> {
    let created = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::AgentTaskCreated {
            task_id: task_id.clone(),
            title: title.to_string(),
            role,
            dependencies: dependencies.to_vec(),
        },
    );
    append_and_broadcast(store, event_tx, &created).await?;

    let started = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::AgentTaskStarted {
            task_id: task_id.clone(),
        },
    );
    append_and_broadcast(store, event_tx, &started).await?;
    Ok(())
}

async fn fail_root_task<S: EventStore + 'static>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    task_graphs: &Arc<Mutex<HashMap<String, TaskGraph>>>,
    request: &SendMessageRequest,
    root_task_id: &TaskId,
    reason: &str,
) {
    {
        let mut guard = task_graphs.lock().await;
        if let Some(graph) = guard.get_mut(&request.session_id.to_string()) {
            let _ = graph.mark_failed(root_task_id, reason.to_string());
        }
    }
    let event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::AgentTaskFailed {
            task_id: root_task_id.clone(),
            error: reason.to_string(),
        },
    );
    let _ = append_and_broadcast(store, event_tx, &event).await;
}

async fn cancel_root_task<S: EventStore + 'static>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    task_graphs: &Arc<Mutex<HashMap<String, TaskGraph>>>,
    request: &SendMessageRequest,
    root_task_id: &TaskId,
) {
    {
        let mut guard = task_graphs.lock().await;
        if let Some(graph) = guard.get_mut(&request.session_id.to_string()) {
            let _ = graph.mark_cancelled(root_task_id);
        }
    }
    let event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::TaskCancelled {
            task_id: root_task_id.clone(),
        },
    );
    let _ = append_and_broadcast(store, event_tx, &event).await;
}

async fn complete_root_task<S: EventStore + 'static>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    task_graphs: &Arc<Mutex<HashMap<String, TaskGraph>>>,
    request: &SendMessageRequest,
    root_task_id: &TaskId,
) {
    {
        let mut guard = task_graphs.lock().await;
        if let Some(graph) = guard.get_mut(&request.session_id.to_string()) {
            let _ = graph.mark_completed(root_task_id);
        }
    }
    let event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::AgentTaskCompleted {
            task_id: root_task_id.clone(),
        },
    );
    let _ = append_and_broadcast(store, event_tx, &event).await;
}

async fn complete_trajectory<S: EventStore + 'static>(
    trajectory_store: &Option<Arc<dyn TrajectoryStore>>,
    store: &Arc<S>,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    request: &SendMessageRequest,
    trajectory_id: &Option<TrajectoryId>,
    step_counter: &std::sync::atomic::AtomicU32,
    outcome: agent_core::TrajectoryOutcome,
) {
    let (Some(ts), Some(tid)) = (trajectory_store.as_ref(), trajectory_id.as_ref()) else {
        return;
    };
    if let Err(e) = ts.complete_trajectory(tid, outcome.clone()).await {
        tracing::warn!("failed to complete trajectory: {e}");
        return;
    }
    let step_count = step_counter.load(std::sync::atomic::Ordering::Relaxed);
    let event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::TrajectoryCompleted {
            trajectory_id: tid.to_string(),
            step_count,
            outcome,
        },
    );
    let _ = append_and_broadcast(&**store, event_tx, &event).await;
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;

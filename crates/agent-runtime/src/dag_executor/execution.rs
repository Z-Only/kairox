use crate::agents::{AgentDecision, AgentStrategy, StepContext};
use crate::dag_executor::events::EventEmitter;
use crate::event_emitter::append_and_broadcast;
use crate::task_graph::{AgentTask, TaskGraph};
use agent_core::{
    AgentId, AgentRole, DomainEvent, EventPayload, PrivacyClassification, TaskId, TaskState,
    WorkspaceId,
};
use agent_models::ModelClient;
use agent_store::EventStore;
use agent_tools::{parse_legacy_mode, ApprovalPolicy, PermissionEngine, SandboxPolicy};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Execute a single task using its assigned strategy.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_task_with_strategy<S, M>(
    events: &EventEmitter<S>,
    model: &Arc<M>,
    strategies: &HashMap<AgentRole, Arc<dyn AgentStrategy>>,
    permission_engine: &Arc<Mutex<PermissionEngine>>,
    workspace_id: &WorkspaceId,
    session_id: &agent_core::SessionId,
    graph: &TaskGraph,
    task: &AgentTask,
    session_events: &[DomainEvent],
    ctx: &StepContext,
    agent_id: &AgentId,
) -> agent_core::Result<()>
where
    S: EventStore,
    M: ModelClient,
{
    let strategy = strategies.get(&task.role).ok_or_else(|| {
        agent_core::CoreError::InvalidState(format!(
            "No strategy registered for role {:?}",
            task.role
        ))
    })?;

    // Apply per-agent policy if the strategy provides an override. Strategies
    // still report a legacy `permission_mode` string from agent settings; we
    // translate that into the canonical `(approval, sandbox)` pair on the way
    // in and snapshot the previous pair so we can restore it after the task.
    let previous_policy: Option<(ApprovalPolicy, SandboxPolicy)> =
        if let Some(mode_str) = strategy.permission_mode_override() {
            if let Some((approval, sandbox)) = parse_legacy_mode(mode_str) {
                let mut engine = permission_engine.lock().await;
                let prev = (engine.approval_policy(), engine.sandbox_policy().clone());
                engine.set_approval_policy(approval);
                engine.set_sandbox_policy(sandbox);
                Some(prev)
            } else {
                None
            }
        } else {
            None
        };

    let messages = strategy.build_context(task, graph, session_events).await;
    let decision = strategy.decide(ctx, messages).await;

    let result = match decision {
        AgentDecision::Respond(text) => {
            let event = DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                agent_id.clone(),
                PrivacyClassification::FullTrace,
                EventPayload::AssistantMessageCompleted {
                    message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                    content: text,
                },
            );
            append_and_broadcast(&*events.store, &events.event_tx, &event).await?;
            Ok(())
        }
        AgentDecision::RequestModel { .. } => {
            // Agent-specific model profile override takes precedence over session-level.
            let model_profile = strategy
                .model_profile_override()
                .map(|s| s.to_string())
                .unwrap_or_else(|| crate::agent_loop::latest_model_profile_for(session_events));
            let reasoning_effort =
                crate::agent_loop::latest_model_reasoning_effort_for(session_events);

            let model_request = agent_models::ModelRequest {
                model_profile,
                messages: strategy.build_context(task, graph, session_events).await,
                system_prompt: None,
                tools: Vec::new(),
                reasoning_effort,
            };

            let mut stream = model
                .stream(model_request)
                .await
                .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

            let mut response_text = String::new();
            use futures::StreamExt;
            while let Some(event_result) = stream.next().await {
                match event_result {
                    Ok(agent_models::ModelEvent::TokenDelta(delta)) => {
                        response_text.push_str(&delta);
                        let event = DomainEvent::new(
                            workspace_id.clone(),
                            session_id.clone(),
                            agent_id.clone(),
                            PrivacyClassification::FullTrace,
                            EventPayload::ModelTokenDelta { delta },
                        );
                        append_and_broadcast(&*events.store, &events.event_tx, &event).await?;
                    }
                    Ok(agent_models::ModelEvent::Completed { .. }) => break,
                    Ok(agent_models::ModelEvent::Failed { message }) => {
                        return Err(agent_core::CoreError::InvalidState(message));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        return Err(agent_core::CoreError::InvalidState(e.to_string()));
                    }
                }
            }

            if !response_text.is_empty() {
                let event = DomainEvent::new(
                    workspace_id.clone(),
                    session_id.clone(),
                    agent_id.clone(),
                    PrivacyClassification::FullTrace,
                    EventPayload::AssistantMessageCompleted {
                        message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                        content: response_text,
                    },
                );
                append_and_broadcast(&*events.store, &events.event_tx, &event).await?;
            }

            Ok(())
        }
        AgentDecision::Decompose { .. } => Err(agent_core::CoreError::InvalidState(
            "Nested decomposition is not yet supported".into(),
        )),
        AgentDecision::ReviewComplete { approved, findings } => {
            for finding in &findings {
                let event = DomainEvent::new(
                    workspace_id.clone(),
                    session_id.clone(),
                    agent_id.clone(),
                    PrivacyClassification::FullTrace,
                    EventPayload::ReviewerFindingAdded {
                        finding_id: format!("finding_{}", uuid::Uuid::new_v4().simple()),
                        severity: finding.severity.clone(),
                        message: finding.message.clone(),
                    },
                );
                append_and_broadcast(&*events.store, &events.event_tx, &event).await?;
            }

            if approved {
                Ok(())
            } else {
                Err(agent_core::CoreError::InvalidState(format!(
                    "Review not approved: {} findings",
                    findings.len()
                )))
            }
        }
    };

    // Restore previous policy pair if it was overridden.
    if let Some((prev_approval, prev_sandbox)) = previous_policy {
        let mut engine = permission_engine.lock().await;
        engine.set_approval_policy(prev_approval);
        engine.set_sandbox_policy(prev_sandbox);
    }

    result
}

/// Run the reviewer on completed worker outputs, if a reviewer task exists.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_reviewer_if_needed<S, M>(
    events: &EventEmitter<S>,
    model: &Arc<M>,
    strategies: &HashMap<AgentRole, Arc<dyn AgentStrategy>>,
    permission_engine: &Arc<Mutex<PermissionEngine>>,
    workspace_id: &WorkspaceId,
    session_id: &agent_core::SessionId,
    graph: &mut TaskGraph,
    session_events: &[DomainEvent],
    ctx: &StepContext,
) -> agent_core::Result<()>
where
    S: EventStore,
    M: ModelClient,
{
    // Find reviewer tasks that are ready to run
    let reviewer_tasks: Vec<TaskId> = graph
        .snapshot()
        .iter()
        .filter(|t| t.role == AgentRole::Reviewer && t.state == TaskState::Pending)
        .filter(|t| {
            t.dependencies.iter().all(|dep| {
                graph
                    .get_task(dep)
                    .map(|d| d.state == TaskState::Completed)
                    .unwrap_or(false)
            })
        })
        .map(|t| t.id.clone())
        .collect();

    for reviewer_task_id in reviewer_tasks {
        graph.mark_running(&reviewer_task_id).unwrap();
        events
            .emit_task_started(workspace_id, session_id, &reviewer_task_id)
            .await?;

        let reviewer_agent_id = AgentId::reviewer();
        events
            .emit_agent_spawned(
                workspace_id,
                session_id,
                &reviewer_agent_id,
                AgentRole::Reviewer,
                &reviewer_task_id,
            )
            .await?;

        let task = graph
            .get_task(&reviewer_task_id)
            .cloned()
            .ok_or_else(|| agent_core::CoreError::InvalidState("Reviewer task not found".into()))?;

        let result = execute_task_with_strategy(
            events,
            model,
            strategies,
            permission_engine,
            workspace_id,
            session_id,
            graph,
            &task,
            session_events,
            ctx,
            &reviewer_agent_id,
        )
        .await;

        match result {
            Ok(()) => {
                graph.mark_completed(&reviewer_task_id).unwrap();
                events
                    .emit_task_completed(workspace_id, session_id, &reviewer_task_id)
                    .await?;
            }
            Err(e) => {
                let error = e.to_string();
                graph.mark_failed(&reviewer_task_id, error.clone()).unwrap();
                events
                    .emit_task_failed(workspace_id, session_id, &reviewer_task_id, &error)
                    .await?;
            }
        }

        events
            .emit_agent_idle(workspace_id, session_id, &reviewer_agent_id)
            .await?;
    }

    Ok(())
}

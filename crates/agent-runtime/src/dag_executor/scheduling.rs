use crate::agents::{AgentStrategy, StepContext};
use crate::dag_executor::events::EventEmitter;
use crate::dag_executor::execution::execute_task_with_strategy;
use crate::dag_executor::recovery::apply_failure_policy;
use crate::event_emitter::append_and_broadcast;
use crate::task_graph::TaskGraph;
use agent_core::{
    AgentId, AgentRole, DomainEvent, EventPayload, PrivacyClassification, TaskFailureReason,
    TaskId, WorkspaceId,
};
use agent_models::ModelClient;
use agent_store::EventStore;
use agent_tools::PermissionEngine;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutionBatch {
    pub(crate) task_ids: Vec<TaskId>,
    pub(crate) capacity: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SchedulingOutcome {
    pub(crate) cancelled: bool,
}

pub(crate) fn next_execution_batch(graph: &TaskGraph, max_concurrency: usize) -> ExecutionBatch {
    let capacity = max_concurrency.max(1);
    let mut task_ids = graph.ready_tasks();
    task_ids.truncate(capacity);
    ExecutionBatch { task_ids, capacity }
}

/// Run the scheduling loop until all tasks are in terminal states.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_scheduling_loop<S, M>(
    events: &EventEmitter<S>,
    model: &Arc<M>,
    strategies: &HashMap<AgentRole, Arc<dyn AgentStrategy>>,
    permission_engine: &Arc<Mutex<PermissionEngine>>,
    config: &super::config::DagConfig,
    model_config: &agent_config::Config,
    workspace_id: &WorkspaceId,
    session_id: &agent_core::SessionId,
    graph: &mut TaskGraph,
    session_events: &[DomainEvent],
    ctx: &StepContext,
    cancellation: Option<&CancellationToken>,
) -> agent_core::Result<SchedulingOutcome>
where
    S: EventStore,
    M: ModelClient,
{
    let mut iteration = 0;
    let max_iterations = 100; // Safety guard

    loop {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            cancel_non_terminal_tasks(events, workspace_id, session_id, graph).await?;
            return Ok(SchedulingOutcome { cancelled: true });
        }

        if iteration >= max_iterations {
            tracing::warn!("DAG scheduling loop exceeded max iterations");
            break;
        }
        iteration += 1;

        let diagnostics = graph.readiness_diagnostics();
        if diagnostics.ready.is_empty() {
            if graph.is_finished() {
                break;
            }
            if diagnostics.running.is_empty() {
                // Deadlock: no ready tasks, no running tasks, but not finished
                tracing::warn!(
                    waiting_tasks = diagnostics.waiting.len(),
                    blocked_tasks = diagnostics.blocked.len(),
                    "DAG scheduling loop stalled with no runnable tasks"
                );
                break;
            }
            // Give running tasks time to complete
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            continue;
        }

        let batch = next_execution_batch(graph, config.max_concurrency);

        // For now, execute tasks sequentially within the loop.
        // Parallel execution with JoinSet + Semaphore will be added
        // once we have async tool execution properly wired.
        for task_id in batch.task_ids {
            if let Some(task) = graph.get_task(&task_id).cloned() {
                graph.mark_running(&task_id).unwrap();
                events
                    .emit_task_started(workspace_id, session_id, &task_id)
                    .await?;

                // Spawn an agent for this task
                let agent_id = AgentId::worker(format!("w_{}", task_id));
                events
                    .emit_agent_spawned(workspace_id, session_id, &agent_id, task.role, &task_id)
                    .await?;

                // Get the strategy for this role
                let result = if strategies.contains_key(&task.role) {
                    execute_task_with_strategy(
                        events,
                        model,
                        strategies,
                        permission_engine,
                        model_config,
                        workspace_id,
                        session_id,
                        graph,
                        &task,
                        session_events,
                        ctx,
                        &agent_id,
                        cancellation,
                    )
                    .await
                } else {
                    // No strategy for this role — mark as failed
                    let error = format!("No strategy registered for role {:?}", task.role);
                    graph.mark_failed(&task_id, error.clone()).unwrap();
                    events
                        .emit_task_failed(workspace_id, session_id, &task_id, &error)
                        .await?;
                    continue;
                };

                if cancellation.is_some_and(CancellationToken::is_cancelled) {
                    cancel_non_terminal_tasks(events, workspace_id, session_id, graph).await?;
                    events
                        .emit_agent_idle(workspace_id, session_id, &agent_id)
                        .await?;
                    return Ok(SchedulingOutcome { cancelled: true });
                }

                match result {
                    Ok(()) => {
                        graph.mark_completed(&task_id).unwrap();
                        events
                            .emit_task_completed(workspace_id, session_id, &task_id)
                            .await?;
                    }
                    Err(e) => {
                        let error = e.to_string();
                        graph
                            .mark_failed_with_reason(
                                &task_id,
                                error.clone(),
                                TaskFailureReason::ModelError { retries: 0 },
                            )
                            .unwrap();
                        events
                            .emit_task_failed(workspace_id, session_id, &task_id, &error)
                            .await?;

                        // Apply failure policy
                        apply_failure_policy(
                            events,
                            config,
                            workspace_id,
                            session_id,
                            graph,
                            &task_id,
                        )
                        .await?;
                    }
                }

                events
                    .emit_agent_idle(workspace_id, session_id, &agent_id)
                    .await?;
            }
        }
    }

    Ok(SchedulingOutcome { cancelled: false })
}

pub(crate) async fn cancel_non_terminal_tasks<S: EventStore>(
    events: &EventEmitter<S>,
    workspace_id: &WorkspaceId,
    session_id: &agent_core::SessionId,
    graph: &mut TaskGraph,
) -> agent_core::Result<()> {
    let cancellable_task_ids: Vec<TaskId> = graph
        .snapshot()
        .into_iter()
        .filter(|task| !task.state.is_terminal())
        .map(|task| task.id)
        .collect();

    for task_id in cancellable_task_ids {
        graph
            .mark_cancelled(&task_id)
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        events
            .emit_task_cancelled(workspace_id, session_id, &task_id)
            .await?;
    }

    Ok(())
}

/// Handle the decomposition of a task into sub-tasks.
pub(crate) async fn handle_decomposition<S: EventStore>(
    events: &EventEmitter<S>,
    config: &super::config::DagConfig,
    workspace_id: &WorkspaceId,
    session_id: &agent_core::SessionId,
    parent_task_id: &TaskId,
    sub_tasks: &[crate::agents::SubTaskDef],
    graph: &mut TaskGraph,
) -> agent_core::Result<()> {
    // First pass: create all tasks and build title→TaskId mapping
    let mut title_to_id: HashMap<String, TaskId> = HashMap::new();

    for sub_task in sub_tasks {
        let task_id = graph.add_task_with_config(
            &sub_task.title,
            sub_task.role,
            Vec::new(), // dependencies resolved in second pass
            config.retry_config.max_tool_retries,
            None,
        );

        // Set description on the task
        if let Some(task) = graph.get_task_mut(&task_id) {
            task.description = sub_task.description.clone();
        }

        title_to_id.insert(sub_task.title.clone(), task_id.clone());

        // Emit task created event
        events
            .emit_task_created(
                workspace_id,
                session_id,
                &task_id,
                &sub_task.title,
                sub_task.role,
                std::slice::from_ref(parent_task_id),
            )
            .await?;
    }

    // Second pass: resolve dependencies by title
    for sub_task in sub_tasks {
        if let Some(task_id) = title_to_id.get(&sub_task.title) {
            let resolved_deps: Vec<TaskId> = sub_task
                .dependencies
                .iter()
                .filter_map(|dep_title| {
                    // First try to find by TaskId (already resolved)
                    graph
                        .get_task(dep_title)
                        .map(|_| dep_title.clone())
                        .or_else(|| title_to_id.get(&dep_title.to_string()).cloned())
                })
                .collect();

            if let Some(task) = graph.get_task_mut(task_id) {
                task.dependencies = resolved_deps;
            }
        }
    }

    // Emit TaskDecomposed event
    let sub_task_ids: Vec<TaskId> = title_to_id.values().cloned().collect();
    let event = DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::planner(),
        PrivacyClassification::MinimalTrace,
        EventPayload::TaskDecomposed {
            parent_task_id: parent_task_id.clone(),
            sub_task_ids,
        },
    );
    append_and_broadcast(&*events.store, &events.event_tx, &event).await?;

    Ok(())
}

#[cfg(test)]
#[path = "scheduling_tests.rs"]
mod tests;

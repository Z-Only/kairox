use crate::dag_executor::events::EventEmitter;
use crate::event_emitter::append_and_broadcast;
use crate::task_graph::TaskGraph;
use agent_core::{
    AgentId, DomainEvent, EventPayload, FailurePolicy, PrivacyClassification, TaskId, TaskState,
    WorkspaceId,
};
use agent_store::EventStore;

/// Apply failure policy when a task fails.
pub(crate) async fn apply_failure_policy<S: EventStore>(
    events: &EventEmitter<S>,
    config: &super::config::DagConfig,
    workspace_id: &WorkspaceId,
    session_id: &agent_core::SessionId,
    graph: &mut TaskGraph,
    failed_task_id: &TaskId,
) -> agent_core::Result<()> {
    match config.failure_policy {
        FailurePolicy::BlockDependents => {
            let dependents = graph.find_blocked_dependents(failed_task_id);
            for dep_id in &dependents {
                if let Some(dep) = graph.get_task(dep_id) {
                    if !dep.state.is_terminal() {
                        graph
                            .mark_blocked(dep_id, format!("dependency {} failed", failed_task_id))
                            .ok();
                        events
                            .emit_task_blocked(
                                workspace_id,
                                session_id,
                                dep_id,
                                failed_task_id,
                                "dependency failed",
                            )
                            .await?;
                    }
                }
            }
        }
        FailurePolicy::AllowOrphans => {
            // Dependents can proceed — they'll receive "parent failed" context
        }
        FailurePolicy::FailFast => {
            // Cancel all non-terminal tasks
            for task in graph.snapshot() {
                if !task.state.is_terminal() && task.id != *failed_task_id {
                    graph.mark_cancelled(&task.id).ok();
                }
            }
        }
    }
    Ok(())
}

/// Retry a previously failed task, resetting it to pending and unblocking dependents.
pub(crate) async fn retry_task<S: EventStore>(
    events: &EventEmitter<S>,
    workspace_id: &WorkspaceId,
    session_id: &agent_core::SessionId,
    graph: &mut TaskGraph,
    task_id: &TaskId,
) -> agent_core::Result<()> {
    let task = graph.get_task(task_id).cloned().ok_or_else(|| {
        agent_core::CoreError::InvalidState(format!("Task {} not found", task_id))
    })?;

    if task.state != TaskState::Failed && task.state != TaskState::Blocked {
        return Err(agent_core::CoreError::InvalidState(format!(
            "Task {} is in state {:?}, can only retry Failed or Blocked tasks",
            task_id, task.state
        )));
    }

    let new_attempt = task.retry_count + 1;
    graph
        .reset_to_pending(task_id)
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

    // Also reset any blocked dependents
    let dependents = graph.find_blocked_dependents(task_id);
    for dep_id in &dependents {
        if let Some(dep) = graph.get_task(dep_id) {
            if dep.state == TaskState::Blocked {
                graph.reset_to_pending(dep_id).ok();
            }
        }
    }

    // Emit TaskRetried event
    let event = DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::TaskRetried {
            task_id: task_id.clone(),
            attempt: new_attempt,
        },
    );
    append_and_broadcast(&*events.store, &events.event_tx, &event).await?;

    Ok(())
}

/// Cancel a specific task.
pub(crate) async fn cancel_task<S: EventStore>(
    events: &EventEmitter<S>,
    config: &super::config::DagConfig,
    workspace_id: &WorkspaceId,
    session_id: &agent_core::SessionId,
    graph: &mut TaskGraph,
    task_id: &TaskId,
) -> agent_core::Result<()> {
    graph
        .mark_cancelled(task_id)
        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

    // Emit TaskCancelled event
    let event = DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::TaskCancelled {
            task_id: task_id.clone(),
        },
    );
    append_and_broadcast(&*events.store, &events.event_tx, &event).await?;

    // Apply failure policy for dependents
    apply_failure_policy(events, config, workspace_id, session_id, graph, task_id).await?;

    Ok(())
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;

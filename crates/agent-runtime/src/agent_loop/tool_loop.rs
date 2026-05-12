use crate::event_emitter::append_and_broadcast;
use crate::task_graph::TaskGraph;
use agent_core::{
    AgentId, AgentRole, DomainEvent, EventPayload, PermissionDecision, PrivacyClassification,
    SessionId, TaskId, WorkspaceId,
};
use agent_models::ToolCall;
use agent_store::EventStore;
use agent_tools::{PermissionEngine, ToolInvocation, ToolRegistry, ToolRisk};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Result of executing a batch of tool calls.
pub(crate) struct ToolLoopResult {
    /// Formatted tool results ready to be appended to a ModelRequest via
    /// `add_tool_result`. Each entry is `(tool_call_id, output_text)`.
    pub(crate) tool_results: Vec<(String, String)>,
}

/// Execute a batch of tool calls through permission checking, sub-task
/// creation, invocation, and event emission.
///
/// This is extracted from `run_agent_loop`'s inner tool-calling loop and
/// must remain FUNCTIONALLY IDENTICAL to the original inline code.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_tool_calls<S: EventStore + 'static>(
    tool_calls: &[ToolCall],
    tool_registry: &Arc<Mutex<ToolRegistry>>,
    permission_engine: &Arc<Mutex<PermissionEngine>>,
    store: &Arc<S>,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    workspace_id: &WorkspaceId,
    session_id: &SessionId,
    pending_permissions: &Arc<
        Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>,
    >,
    task_graphs: &Arc<Mutex<HashMap<String, TaskGraph>>>,
    root_task_id: &TaskId,
) -> agent_core::Result<ToolLoopResult> {
    let mut tool_results: Vec<(String, String)> = Vec::new();

    // Process tool calls through permission and execution
    let registry = tool_registry.lock().await;
    for tc in tool_calls {
        // Check permission
        let risk = if let Some(tool) = registry.get(&tc.name).await {
            let inv = ToolInvocation {
                tool_id: tc.name.clone(),
                arguments: tc.arguments.clone(),
                workspace_id: workspace_id.to_string(),
                preview: format!("{}({})", tc.name, tc.arguments),
                timeout_ms: 30_000,
                output_limit_bytes: 102_400,
            };
            tool.risk(&inv)
        } else {
            ToolRisk::read(&tc.name)
        };

        let preview = format!("{}({})", tc.name, tc.arguments);
        let perm_result = crate::permission::check_tool_permission(
            &**store,
            event_tx,
            permission_engine,
            pending_permissions,
            workspace_id,
            session_id,
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
                if let Some(graph) = task_graphs_guard.get_mut(&session_id.to_string()) {
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
                    workspace_id.clone(),
                    session_id.clone(),
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
                    workspace_id.clone(),
                    session_id.clone(),
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
                workspace_id: workspace_id.to_string(),
                preview: format!("{}({})", tc.name, tc.arguments),
                timeout_ms: 30_000,
                output_limit_bytes: 102_400,
            };

            let tool_start = std::time::Instant::now();

            let start_event = DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
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
                Ok(ref output) => DomainEvent::new(
                    workspace_id.clone(),
                    session_id.clone(),
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
                Err(ref e) => DomainEvent::new(
                    workspace_id.clone(),
                    session_id.clone(),
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

            // Collect tool result for the next model request
            let result_text = match &completion_event.payload {
                EventPayload::ToolInvocationCompleted { .. } => {
                    format!(
                        "tool_id={}\nresult={}",
                        tc.name,
                        result.as_ref().unwrap().text
                    )
                }
                EventPayload::ToolInvocationFailed { error, .. } => {
                    format!("tool_id={}\nresult=Error: {}", tc.name, error)
                }
                _ => unreachable!(),
            };
            tool_results.push((tc.id.clone(), result_text));

            // Mark sub-task as completed or failed
            if let Some(sub_id) = sub_task_id {
                let task_event = match &completion_event.payload {
                    EventPayload::ToolInvocationCompleted { .. } => {
                        {
                            let mut task_graphs_guard = task_graphs.lock().await;
                            if let Some(graph) = task_graphs_guard.get_mut(&session_id.to_string())
                            {
                                let _ = graph.mark_completed(&sub_id);
                            }
                        }
                        Some(DomainEvent::new(
                            workspace_id.clone(),
                            session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::MinimalTrace,
                            EventPayload::AgentTaskCompleted { task_id: sub_id },
                        ))
                    }
                    EventPayload::ToolInvocationFailed { error, .. } => {
                        {
                            let mut task_graphs_guard = task_graphs.lock().await;
                            if let Some(graph) = task_graphs_guard.get_mut(&session_id.to_string())
                            {
                                let _ = graph.mark_failed(&sub_id, error.clone());
                            }
                        }
                        Some(DomainEvent::new(
                            workspace_id.clone(),
                            session_id.clone(),
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
        } else {
            // Permission denied — add a fallback result so the model knows the
            // tool was not executed.
            tool_results.push((
                tc.id.clone(),
                format!(
                    "tool_id={}\nresult=Error: Permission denied by user",
                    tc.name
                ),
            ));
        }
    }
    drop(registry);

    Ok(ToolLoopResult { tool_results })
}

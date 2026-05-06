//! Agent loop logic extracted from the runtime facade.
//!
//! This module contains the core orchestrating loop that drives the
//! model → tool-call → permission → execute → feed-back cycle, as well as
//! the helper that converts session history into model messages.

use crate::event_emitter::append_and_broadcast;
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

    // First pass: collect tool call requests and results
    for event in session_events {
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

    // Second pass: build messages with proper tool_calls and tool_call_id
    let mut tool_call_idx = 0;
    for event in session_events {
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

#[allow(clippy::too_many_arguments)]
pub async fn run_agent_loop<S, M>(
    store: &Arc<S>,
    model: &Arc<M>,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    tool_registry: &Arc<Mutex<ToolRegistry>>,
    permission_engine: &Arc<Mutex<PermissionEngine>>,
    pending_permissions: &Arc<
        Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>,
    >,
    memory_store: &Option<Arc<dyn MemoryStore>>,
    task_graphs: &Arc<Mutex<HashMap<String, TaskGraph>>>,
    active_cancellation: &Arc<Mutex<Option<CancellationToken>>>,
    request: &SendMessageRequest,
) -> agent_core::Result<()>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
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

    let messages = build_model_messages(&request.content, &session_events);

    // Inject registered tool definitions into model request
    let tool_defs = {
        let registry = tool_registry.lock().await;
        let definitions = registry.list_all().await;
        definitions
            .into_iter()
            .map(|td| agent_models::ToolDefinition {
                name: td.tool_id,
                description: td.description,
                parameters: td.parameters,
            })
            .collect()
    };

    // Use the session's model profile to route to the correct model client.
    // The model profile is recorded in the SessionInitialized event.
    // Fall back to "fake" for backward compatibility with pre-0.7 sessions.
    let model_profile = session_events
        .iter()
        .find_map(|e| match &e.payload {
            EventPayload::SessionInitialized { model_profile } => Some(model_profile.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "fake".to_string());

    // Retrieve relevant memories from the MemoryStore and inject them
    // into the system prompt so the model can use prior context.
    let mut system_prompt = SYSTEM_PROMPT.to_string();
    if let Some(section) =
        crate::memory_handler::retrieve_memory_section(memory_store, &request.content).await
    {
        system_prompt.push_str(&section);
    }

    let model_request = ModelRequest {
        model_profile,
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
                Ok(ModelEvent::Completed { .. }) => {
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

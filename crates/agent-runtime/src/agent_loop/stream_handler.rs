use crate::agent_loop::AgentLoopDeps;
use crate::event_emitter::append_and_broadcast;
use agent_core::{AgentId, DomainEvent, EventPayload, PrivacyClassification, TaskId};
use agent_memory::strip_memory_markers;
use agent_models::ToolCall;
use agent_store::EventStore;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

const EMPTY_MODEL_RESPONSE_ERROR: &str =
    "model returned an empty response; check model availability, quota, or plan";

const MODEL_STREAM_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Output from processing a single model stream.
pub(crate) struct StreamOutput {
    pub(crate) assistant_text: String,
    pub(crate) tool_calls: Vec<ToolCall>,
}

/// Process the model's event stream for one turn iteration.
///
/// Handles TokenDelta, ToolCallRequested, Completed, Failed, and error
/// events. Broadcasts each event via `event_tx` and stores it in `store`.
/// Returns the accumulated assistant text and any tool calls the model
/// requested. On stream or model failure, emits failure events and
/// returns `Err`.
pub(crate) async fn process_model_stream<S, M>(
    deps: &AgentLoopDeps<'_, S, M>,
    request: &agent_core::SendMessageRequest,
    cancel_token: &CancellationToken,
    root_task_id: &TaskId,
    current_request: &agent_models::ModelRequest,
    empty_response_fallback: Option<&str>,
) -> agent_core::Result<StreamOutput>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    process_model_stream_with_idle_timeout(
        deps,
        request,
        cancel_token,
        root_task_id,
        current_request,
        empty_response_fallback,
        MODEL_STREAM_IDLE_TIMEOUT,
    )
    .await
}

async fn process_model_stream_with_idle_timeout<S, M>(
    deps: &AgentLoopDeps<'_, S, M>,
    request: &agent_core::SendMessageRequest,
    cancel_token: &CancellationToken,
    root_task_id: &TaskId,
    current_request: &agent_models::ModelRequest,
    empty_response_fallback: Option<&str>,
    idle_timeout: std::time::Duration,
) -> agent_core::Result<StreamOutput>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    let stream_result = tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            return Ok(StreamOutput {
                assistant_text: String::new(),
                tool_calls: Vec::new(),
            });
        }
        result = deps.model.stream(current_request.clone()) => result,
        _ = tokio::time::sleep(idle_timeout) => {
            let error_msg = model_stream_timeout_error(idle_timeout);
            emit_model_request_failure(
                &**deps.store,
                deps.event_tx,
                request,
                root_task_id,
                deps.task_graphs,
                &error_msg,
            )
            .await;
            return Err(agent_core::CoreError::InvalidState(error_msg));
        }
    };

    let mut stream = match stream_result {
        Ok(s) => s,
        Err(e) => {
            let error_msg = e.to_string();
            emit_model_request_failure(
                &**deps.store,
                deps.event_tx,
                request,
                root_task_id,
                deps.task_graphs,
                &error_msg,
            )
            .await;
            return Err(agent_core::CoreError::InvalidState(error_msg));
        }
    };

    let mut assistant_text = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    loop {
        let event_result = tokio::select! {
            biased;
            _ = cancel_token.cancelled() => break,
            _ = tokio::time::sleep(idle_timeout) => {
                let error_msg = model_stream_timeout_error(idle_timeout);
                emit_model_request_failure(
                    &**deps.store,
                    deps.event_tx,
                    request,
                    root_task_id,
                    deps.task_graphs,
                    &error_msg,
                )
                .await;
                return Err(agent_core::CoreError::InvalidState(error_msg));
            }
            event = stream.next() => {
                let Some(event_result) = event else {
                    break;
                };
                event_result
            }
        };

        match event_result {
            Ok(agent_models::ModelEvent::TokenDelta(delta)) => {
                assistant_text.push_str(&delta);
                let event = DomainEvent::new(
                    request.workspace_id.clone(),
                    request.session_id.clone(),
                    AgentId::system(),
                    PrivacyClassification::FullTrace,
                    EventPayload::ModelTokenDelta { delta },
                );
                append_and_broadcast(&**deps.store, deps.event_tx, &event).await?;
                if cancel_token.is_cancelled() {
                    break;
                }
            }
            Ok(agent_models::ModelEvent::ToolCallRequested {
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
                append_and_broadcast(&**deps.store, deps.event_tx, &event).await?;
                tool_calls.push(ToolCall {
                    id: tool_call_id,
                    name: tool_id,
                    arguments,
                });
            }
            Ok(agent_models::ModelEvent::Completed { usage: real_usage }) => {
                if let Some(u) = real_usage {
                    let mut states = deps.session_states.lock().await;
                    if let Some(entry) = states.get_mut(request.session_id.as_str()) {
                        let estimated = entry.last_estimated_tokens;
                        if estimated > 0 {
                            entry.usage_corrector.update(u.input_tokens, estimated);
                        }
                    }
                }
                let display_content = if assistant_text.is_empty() {
                    String::new()
                } else {
                    strip_memory_markers(&assistant_text)
                };
                if !display_content.is_empty() {
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
                    append_and_broadcast(&**deps.store, deps.event_tx, &event).await?;
                }
            }
            Ok(agent_models::ModelEvent::Failed { message }) => {
                emit_model_request_failure(
                    &**deps.store,
                    deps.event_tx,
                    request,
                    root_task_id,
                    deps.task_graphs,
                    &message,
                )
                .await;
                return Err(agent_core::CoreError::InvalidState(message));
            }
            Err(e) => {
                let error_msg = e.to_string();
                emit_model_request_failure(
                    &**deps.store,
                    deps.event_tx,
                    request,
                    root_task_id,
                    deps.task_graphs,
                    &error_msg,
                )
                .await;
                return Err(agent_core::CoreError::InvalidState(error_msg));
            }
        }
    }

    if !cancel_token.is_cancelled() && assistant_text.trim().is_empty() && tool_calls.is_empty() {
        if let Some(fallback) = empty_response_fallback {
            let event = DomainEvent::new(
                request.workspace_id.clone(),
                request.session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::AssistantMessageCompleted {
                    message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                    content: fallback.to_string(),
                },
            );
            append_and_broadcast(&**deps.store, deps.event_tx, &event).await?;
            return Ok(StreamOutput {
                assistant_text: fallback.to_string(),
                tool_calls,
            });
        }

        emit_model_request_failure(
            &**deps.store,
            deps.event_tx,
            request,
            root_task_id,
            deps.task_graphs,
            EMPTY_MODEL_RESPONSE_ERROR,
        )
        .await;
        return Err(agent_core::CoreError::InvalidState(
            EMPTY_MODEL_RESPONSE_ERROR.to_string(),
        ));
    }

    Ok(StreamOutput {
        assistant_text,
        tool_calls,
    })
}

fn model_stream_timeout_error(timeout: std::time::Duration) -> String {
    let timeout_ms = timeout.as_millis();
    if timeout_ms >= 1_000 && timeout_ms.is_multiple_of(1_000) {
        format!(
            "model stream timed out after {}s without producing an event",
            timeout_ms / 1_000
        )
    } else {
        format!("model stream timed out after {timeout_ms}ms without producing an event")
    }
}

async fn emit_model_request_failure<S: EventStore + 'static>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    request: &agent_core::SendMessageRequest,
    root_task_id: &TaskId,
    task_graphs: &Arc<Mutex<HashMap<String, crate::task_graph::TaskGraph>>>,
    error_msg: &str,
) {
    let fail_event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::AgentTaskFailed {
            task_id: TaskId::new(),
            error: error_msg.to_string(),
        },
    );
    let _ = append_and_broadcast(store, event_tx, &fail_event).await;
    {
        let mut guard = task_graphs.lock().await;
        if let Some(graph) = guard.get_mut(&request.session_id.to_string()) {
            let _ = graph.mark_failed(root_task_id, error_msg.to_string());
        }
    }
    let root_fail = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::AgentTaskFailed {
            task_id: root_task_id.clone(),
            error: error_msg.to_string(),
        },
    );
    let _ = append_and_broadcast(store, event_tx, &root_fail).await;
}

#[cfg(test)]
#[path = "stream_handler_tests.rs"]
mod tests;

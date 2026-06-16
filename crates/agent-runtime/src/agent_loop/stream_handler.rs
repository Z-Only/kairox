use crate::agent_loop::AgentLoopDeps;
use crate::event_emitter::append_and_broadcast;
use agent_core::{AgentId, DomainEvent, EventPayload, PrivacyClassification, TaskId};
use agent_memory::strip_memory_markers;
use agent_models::ToolCall;
use agent_store::EventStore;
use futures::{stream::BoxStream, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

const EMPTY_MODEL_RESPONSE_ERROR: &str =
    "model returned an empty response; check model availability, quota, or plan";
const MODEL_STREAM_START_IDLE_RETRIES: usize = 1;

/// Output from processing a single model stream.
pub(crate) struct StreamOutput {
    pub(crate) assistant_text: String,
    pub(crate) tool_calls: Vec<ToolCall>,
    pub(crate) empty_response_fallback_used: bool,
}

enum StreamStartResult {
    Opened(BoxStream<'static, agent_models::Result<agent_models::ModelEvent>>),
    ModelError(String),
    Timeout(String),
}

#[derive(Clone, Copy)]
struct ModelStreamProgress {
    phase: &'static str,
    assistant_chars: usize,
    tool_call_count: usize,
    last_event_kind: &'static str,
    retry: Option<ModelStreamRetry>,
}

#[derive(Clone, Copy)]
struct ModelStreamRetry {
    attempt: usize,
    max_retries: usize,
}

impl ModelStreamProgress {
    fn new(
        phase: &'static str,
        assistant_chars: usize,
        tool_call_count: usize,
        last_event_kind: &'static str,
    ) -> Self {
        Self {
            phase,
            assistant_chars,
            tool_call_count,
            last_event_kind,
            retry: None,
        }
    }

    fn retrying(
        phase: &'static str,
        assistant_chars: usize,
        tool_call_count: usize,
        last_event_kind: &'static str,
        attempt: usize,
        max_retries: usize,
    ) -> Self {
        Self {
            phase,
            assistant_chars,
            tool_call_count,
            last_event_kind,
            retry: Some(ModelStreamRetry {
                attempt,
                max_retries,
            }),
        }
    }

    fn is_retrying(self) -> bool {
        self.retry.is_some()
    }

    fn retry_attempt(self) -> usize {
        self.retry.map(|retry| retry.attempt).unwrap_or(0)
    }

    fn max_retries(self) -> usize {
        self.retry.map(|retry| retry.max_retries).unwrap_or(0)
    }
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
        model_stream_idle_timeout(deps.config),
    )
    .await
}

fn model_stream_idle_timeout(config: &agent_config::Config) -> std::time::Duration {
    std::time::Duration::from_secs(
        config
            .context
            .model_stream_idle_timeout_secs
            .unwrap_or(agent_config::DEFAULT_MODEL_STREAM_IDLE_TIMEOUT_SECS),
    )
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
    log_model_stream_start(request, root_task_id, current_request, idle_timeout);

    let mut stream_start_attempt = 0usize;
    let mut stream = loop {
        let stream_result = tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                log_model_stream_cancelled(
                    request,
                    root_task_id,
                    current_request,
                    "stream_start",
                    0,
                    0,
                    "none",
                );
                return Ok(StreamOutput {
                    assistant_text: String::new(),
                    tool_calls: Vec::new(),
                    empty_response_fallback_used: false,
                });
            }
            result = deps.model.stream(current_request.clone()) => match result {
                Ok(stream) => StreamStartResult::Opened(stream),
                Err(error) => StreamStartResult::ModelError(error.to_string()),
            },
            _ = tokio::time::sleep(idle_timeout) => {
                let error_msg = model_stream_timeout_error_with_context(
                    current_request,
                    idle_timeout,
                    "stream_start",
                    0,
                    0,
                    "none",
                );
                StreamStartResult::Timeout(error_msg)
            }
        };

        match stream_result {
            StreamStartResult::Opened(stream) => break stream,
            StreamStartResult::Timeout(_error_msg)
                if stream_start_attempt < MODEL_STREAM_START_IDLE_RETRIES =>
            {
                let progress = ModelStreamProgress::retrying(
                    "stream_start",
                    0,
                    0,
                    "none",
                    stream_start_attempt + 1,
                    MODEL_STREAM_START_IDLE_RETRIES,
                );
                log_model_stream_timeout(
                    request,
                    root_task_id,
                    current_request,
                    idle_timeout,
                    progress,
                );
                emit_model_stream_status(
                    &**deps.store,
                    deps.event_tx,
                    request,
                    progress,
                    model_stream_timeout_log_message(progress),
                )
                .await?;
                stream_start_attempt += 1;
                continue;
            }
            StreamStartResult::Timeout(error_msg) => {
                let progress = ModelStreamProgress::new("stream_start", 0, 0, "none");
                log_model_stream_timeout(
                    request,
                    root_task_id,
                    current_request,
                    idle_timeout,
                    progress,
                );
                emit_model_stream_status(
                    &**deps.store,
                    deps.event_tx,
                    request,
                    progress,
                    error_msg.clone(),
                )
                .await?;
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
            StreamStartResult::ModelError(error_msg) => {
                log_model_stream_failure(
                    request,
                    root_task_id,
                    current_request,
                    &error_msg,
                    ModelStreamProgress::new("stream_start", 0, 0, "none"),
                );
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
    };

    let mut assistant_text = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut last_event_kind = "stream_opened";
    tracing::debug!(
        session_id = request.session_id.as_str(),
        root_task_id = root_task_id.as_str(),
        model_profile = current_request.model_profile.as_str(),
        "model stream opened"
    );

    loop {
        let event_result = tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                log_model_stream_cancelled(
                    request,
                    root_task_id,
                    current_request,
                    "stream_event",
                    assistant_text.len(),
                    tool_calls.len(),
                    last_event_kind,
                );
                break;
            }
            _ = tokio::time::sleep(idle_timeout) => {
                let error_msg = model_stream_timeout_error_with_context(
                    current_request,
                    idle_timeout,
                    "stream_event",
                    assistant_text.len(),
                    tool_calls.len(),
                    last_event_kind,
                );
                let progress = ModelStreamProgress::new(
                    "stream_event",
                    assistant_text.len(),
                    tool_calls.len(),
                    last_event_kind,
                );
                log_model_stream_timeout(
                    request,
                    root_task_id,
                    current_request,
                    idle_timeout,
                    progress,
                );
                emit_model_stream_status(
                    &**deps.store,
                    deps.event_tx,
                    request,
                    progress,
                    error_msg.clone(),
                )
                .await?;
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
                last_event_kind = "token_delta";
                tracing::trace!(
                    session_id = request.session_id.as_str(),
                    root_task_id = root_task_id.as_str(),
                    model_profile = current_request.model_profile.as_str(),
                    delta_chars = delta.len(),
                    assistant_chars = assistant_text.len() + delta.len(),
                    tool_call_count = tool_calls.len(),
                    "model stream token delta"
                );
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
                    log_model_stream_cancelled(
                        request,
                        root_task_id,
                        current_request,
                        "after_token_delta",
                        assistant_text.len(),
                        tool_calls.len(),
                        last_event_kind,
                    );
                    break;
                }
            }
            Ok(agent_models::ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            }) => {
                last_event_kind = "tool_call_requested";
                tracing::debug!(
                    session_id = request.session_id.as_str(),
                    root_task_id = root_task_id.as_str(),
                    model_profile = current_request.model_profile.as_str(),
                    tool_id = tool_id.as_str(),
                    assistant_chars = assistant_text.len(),
                    tool_call_count = tool_calls.len() + 1,
                    "model stream requested tool"
                );
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
                last_event_kind = "completed";
                tracing::debug!(
                    session_id = request.session_id.as_str(),
                    root_task_id = root_task_id.as_str(),
                    model_profile = current_request.model_profile.as_str(),
                    assistant_chars = assistant_text.len(),
                    tool_call_count = tool_calls.len(),
                    input_tokens = real_usage.as_ref().map(|u| u.input_tokens),
                    output_tokens = real_usage.as_ref().map(|u| u.output_tokens),
                    "model stream completed event"
                );
                if let Some(u) = real_usage {
                    let usage_only_progress =
                        assistant_text.is_empty() && tool_calls.is_empty() && u.output_tokens == 0;
                    let mut states = deps.session_states.lock().await;
                    if let Some(entry) = states.get_mut(request.session_id.as_str()) {
                        let estimated = entry.last_estimated_tokens;
                        if estimated > 0 {
                            entry.usage_corrector.update(u.input_tokens, estimated);
                        }
                    }
                    if usage_only_progress {
                        continue;
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
                break;
            }
            Ok(agent_models::ModelEvent::Failed { message }) => {
                last_event_kind = "failed";
                log_model_stream_failure(
                    request,
                    root_task_id,
                    current_request,
                    &message,
                    ModelStreamProgress::new(
                        "stream_event",
                        assistant_text.len(),
                        tool_calls.len(),
                        last_event_kind,
                    ),
                );
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
                log_model_stream_failure(
                    request,
                    root_task_id,
                    current_request,
                    &error_msg,
                    ModelStreamProgress::new(
                        "stream_event",
                        assistant_text.len(),
                        tool_calls.len(),
                        last_event_kind,
                    ),
                );
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

    tracing::debug!(
        session_id = request.session_id.as_str(),
        root_task_id = root_task_id.as_str(),
        model_profile = current_request.model_profile.as_str(),
        assistant_chars = assistant_text.len(),
        tool_call_count = tool_calls.len(),
        cancelled = cancel_token.is_cancelled(),
        last_event_kind,
        "model stream ended"
    );

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
                empty_response_fallback_used: true,
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
        empty_response_fallback_used: false,
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

fn model_stream_timeout_error_with_context(
    model_request: &agent_models::ModelRequest,
    timeout: std::time::Duration,
    phase: &'static str,
    assistant_chars: usize,
    tool_call_count: usize,
    last_event_kind: &'static str,
) -> String {
    let stats = model_request_stats(model_request);
    format!(
        "{} (phase={}, last_event={}, messages={}, tools={}, server_tools={}, tool_results={}, assistant_tool_messages={}, assistant_chars={}, emitted_tool_calls={})",
        model_stream_timeout_error(timeout),
        phase,
        last_event_kind,
        stats.message_count,
        stats.tool_count,
        stats.server_tool_count,
        stats.tool_result_count,
        stats.assistant_tool_call_message_count,
        assistant_chars,
        tool_call_count,
    )
}

struct ModelRequestStats {
    message_count: usize,
    tool_count: usize,
    server_tool_count: usize,
    tool_result_count: usize,
    assistant_tool_call_message_count: usize,
}

fn model_request_stats(request: &agent_models::ModelRequest) -> ModelRequestStats {
    ModelRequestStats {
        message_count: request.messages.len(),
        tool_count: request.tools.len(),
        server_tool_count: request.server_tools.len(),
        tool_result_count: request
            .messages
            .iter()
            .filter(|message| message.role == "tool")
            .count(),
        assistant_tool_call_message_count: request
            .messages
            .iter()
            .filter(|message| message.role == "assistant" && !message.tool_calls.is_empty())
            .count(),
    }
}

fn log_model_stream_start(
    request: &agent_core::SendMessageRequest,
    root_task_id: &TaskId,
    model_request: &agent_models::ModelRequest,
    idle_timeout: std::time::Duration,
) {
    let stats = model_request_stats(model_request);
    tracing::debug!(
        session_id = request.session_id.as_str(),
        root_task_id = root_task_id.as_str(),
        model_profile = model_request.model_profile.as_str(),
        message_count = stats.message_count,
        tool_count = stats.tool_count,
        server_tool_count = stats.server_tool_count,
        tool_result_count = stats.tool_result_count,
        assistant_tool_call_message_count = stats.assistant_tool_call_message_count,
        idle_timeout_ms = idle_timeout.as_millis() as u64,
        "starting model stream"
    );
}

fn log_model_stream_timeout(
    request: &agent_core::SendMessageRequest,
    root_task_id: &TaskId,
    model_request: &agent_models::ModelRequest,
    idle_timeout: std::time::Duration,
    progress: ModelStreamProgress,
) {
    let stats = model_request_stats(model_request);
    let timeout_message = model_stream_timeout_log_message(progress);
    if progress.is_retrying() {
        tracing::warn!(
            session_id = request.session_id.as_str(),
            root_task_id = root_task_id.as_str(),
            model_profile = model_request.model_profile.as_str(),
            phase = progress.phase,
            retrying = true,
            retry_attempt = progress.retry_attempt(),
            max_retries = progress.max_retries(),
            message_count = stats.message_count,
            tool_count = stats.tool_count,
            server_tool_count = stats.server_tool_count,
            tool_result_count = stats.tool_result_count,
            assistant_tool_call_message_count = stats.assistant_tool_call_message_count,
            assistant_chars = progress.assistant_chars,
            tool_call_count = progress.tool_call_count,
            last_event_kind = progress.last_event_kind,
            idle_timeout_ms = idle_timeout.as_millis() as u64,
            timeout_message,
            "model stream start idle timeout; retrying"
        );
    } else {
        tracing::warn!(
            session_id = request.session_id.as_str(),
            root_task_id = root_task_id.as_str(),
            model_profile = model_request.model_profile.as_str(),
            phase = progress.phase,
            retrying = false,
            retry_attempt = 0usize,
            max_retries = 0usize,
            message_count = stats.message_count,
            tool_count = stats.tool_count,
            server_tool_count = stats.server_tool_count,
            tool_result_count = stats.tool_result_count,
            assistant_tool_call_message_count = stats.assistant_tool_call_message_count,
            assistant_chars = progress.assistant_chars,
            tool_call_count = progress.tool_call_count,
            last_event_kind = progress.last_event_kind,
            idle_timeout_ms = idle_timeout.as_millis() as u64,
            timeout_message,
            "model stream idle timeout"
        );
    }
}

fn model_stream_timeout_log_message(progress: ModelStreamProgress) -> &'static str {
    if progress.is_retrying() {
        "model stream start idle timeout; retrying"
    } else {
        "model stream idle timeout"
    }
}

fn log_model_stream_failure(
    request: &agent_core::SendMessageRequest,
    root_task_id: &TaskId,
    model_request: &agent_models::ModelRequest,
    error: &str,
    progress: ModelStreamProgress,
) {
    let stats = model_request_stats(model_request);
    tracing::warn!(
        session_id = request.session_id.as_str(),
        root_task_id = root_task_id.as_str(),
        model_profile = model_request.model_profile.as_str(),
        phase = progress.phase,
        message_count = stats.message_count,
        tool_count = stats.tool_count,
        server_tool_count = stats.server_tool_count,
        tool_result_count = stats.tool_result_count,
        assistant_tool_call_message_count = stats.assistant_tool_call_message_count,
        assistant_chars = progress.assistant_chars,
        tool_call_count = progress.tool_call_count,
        last_event_kind = progress.last_event_kind,
        error = %error,
        "model stream failed"
    );
}

fn log_model_stream_cancelled(
    request: &agent_core::SendMessageRequest,
    root_task_id: &TaskId,
    model_request: &agent_models::ModelRequest,
    phase: &'static str,
    assistant_chars: usize,
    tool_call_count: usize,
    last_event_kind: &'static str,
) {
    let stats = model_request_stats(model_request);
    tracing::debug!(
        session_id = request.session_id.as_str(),
        root_task_id = root_task_id.as_str(),
        model_profile = model_request.model_profile.as_str(),
        phase,
        message_count = stats.message_count,
        tool_count = stats.tool_count,
        server_tool_count = stats.server_tool_count,
        tool_result_count = stats.tool_result_count,
        assistant_tool_call_message_count = stats.assistant_tool_call_message_count,
        assistant_chars,
        tool_call_count,
        last_event_kind,
        "model stream cancelled"
    );
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

async fn emit_model_stream_status<S: EventStore + 'static>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    request: &agent_core::SendMessageRequest,
    progress: ModelStreamProgress,
    message: impl Into<String>,
) -> agent_core::Result<()> {
    let event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::ModelStreamStatus {
            phase: progress.phase.to_string(),
            retrying: progress.is_retrying(),
            retry_attempt: progress.retry_attempt(),
            max_retries: progress.max_retries(),
            message: message.into(),
        },
    );
    append_and_broadcast(store, event_tx, &event).await
}

#[cfg(test)]
#[path = "stream_handler_tests.rs"]
mod tests;

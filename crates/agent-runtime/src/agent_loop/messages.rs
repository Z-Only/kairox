use agent_core::{DomainEvent, EventPayload};

const CANCELLED_TURN_ASSISTANT_MESSAGE: &str = "[The previous response was cancelled by the user. Do not continue or answer that cancelled request unless the user explicitly asks.]";

fn flush_pending_tool_calls(
    messages: &mut Vec<agent_models::ModelMessage>,
    pending_tool_calls: &mut Vec<agent_models::ToolCall>,
) {
    if pending_tool_calls.is_empty() {
        return;
    }

    messages.push(agent_models::ModelMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: std::mem::take(pending_tool_calls),
        tool_call_id: None,
    });
}

fn close_cancelled_turn(
    messages: &mut Vec<agent_models::ModelMessage>,
    pending_tool_calls: &mut Vec<agent_models::ToolCall>,
) {
    pending_tool_calls.clear();
    while messages
        .last()
        .is_some_and(|m| m.role == "assistant" && !m.tool_calls.is_empty())
    {
        messages.pop();
    }

    let should_close_turn = messages
        .last()
        .is_some_and(|message| message.role != "assistant");
    if should_close_turn {
        messages.push(agent_models::ModelMessage {
            role: "assistant".into(),
            content: CANCELLED_TURN_ASSISTANT_MESSAGE.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        });
    }
}

pub fn build_model_messages(
    user_content: &str,
    session_events: &[DomainEvent],
) -> Vec<agent_models::ModelMessage> {
    let mut messages = Vec::new();
    let mut pending_tool_calls: Vec<agent_models::ToolCall> = Vec::new();

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

    // Build messages with proper tool_calls and tool_call_id.
    // Summaries are injected just before the first event whose timestamp
    // is strictly greater than the summary's `last_ts` (so they appear
    // chronologically in place of the replaced range).
    let mut injected: Vec<bool> = vec![false; summaries.len()];
    for event in session_events {
        if matches!(event.payload, EventPayload::CompactionSummary { .. }) {
            continue;
        }
        // Inject any summary whose covered range ends strictly before this event.
        for (idx, (_, last_ts, content)) in summaries.iter().enumerate() {
            if !injected[idx] && event.timestamp > *last_ts {
                flush_pending_tool_calls(&mut messages, &mut pending_tool_calls);
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
                flush_pending_tool_calls(&mut messages, &mut pending_tool_calls);
                messages.push(agent_models::ModelMessage {
                    role: "user".into(),
                    content: content.clone(),
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                });
            }
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
            EventPayload::AssistantMessageCompleted { content, .. } => {
                messages.push(agent_models::ModelMessage {
                    role: "assistant".into(),
                    content: content.clone(),
                    tool_calls: std::mem::take(&mut pending_tool_calls),
                    tool_call_id: None,
                });
            }
            EventPayload::ToolInvocationCompleted {
                invocation_id,
                output_preview,
                ..
            } => {
                flush_pending_tool_calls(&mut messages, &mut pending_tool_calls);
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
                flush_pending_tool_calls(&mut messages, &mut pending_tool_calls);
                messages.push(agent_models::ModelMessage {
                    role: "tool".into(),
                    content: format!("Error: {}", error),
                    tool_calls: Vec::new(),
                    tool_call_id: Some(invocation_id.clone()),
                });
            }
            EventPayload::PermissionDenied { request_id, reason } => {
                flush_pending_tool_calls(&mut messages, &mut pending_tool_calls);
                messages.push(agent_models::ModelMessage {
                    role: "tool".into(),
                    content: format!("Permission denied: {}", reason),
                    tool_calls: Vec::new(),
                    tool_call_id: Some(request_id.clone()),
                });
            }
            EventPayload::SessionCancelled { .. } => {
                close_cancelled_turn(&mut messages, &mut pending_tool_calls);
            }
            _ => {}
        }
    }

    flush_pending_tool_calls(&mut messages, &mut pending_tool_calls);

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

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;

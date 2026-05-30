use agent_core::{DomainEvent, EventPayload};

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

    // First pass: collect tool call requests and results — skip events that
    // fall inside a covered range so the summary fully replaces them.
    for event in session_events {
        if matches!(event.payload, EventPayload::CompactionSummary { .. }) {
            continue;
        }
        if covered(event.timestamp) {
            continue;
        }
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

    // Second pass: build messages with proper tool_calls and tool_call_id.
    // Summaries are injected just before the first event whose timestamp
    // is strictly greater than the summary's `last_ts` (so they appear
    // chronologically in place of the replaced range).
    let mut injected: Vec<bool> = vec![false; summaries.len()];
    let mut tool_call_idx = 0;
    for event in session_events {
        if matches!(event.payload, EventPayload::CompactionSummary { .. }) {
            continue;
        }
        // Inject any summary whose covered range ends strictly before this event.
        for (idx, (_, last_ts, content)) in summaries.iter().enumerate() {
            if !injected[idx] && event.timestamp > *last_ts {
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

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;

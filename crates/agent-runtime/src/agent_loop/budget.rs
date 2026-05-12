use agent_core::DomainEvent;

use super::build_model_messages;

/// Decide whether the agent loop should fire an auto-compaction request
/// for this iteration. Pure function so it's trivial to unit-test the
/// boundary cases (threshold == 1.0 disables; busy gate skips; exact
/// equality counts as crossing the threshold per spec §4.4).
pub fn should_trigger_auto_compaction(
    usage: &agent_core::ContextUsage,
    threshold: f32,
    already_compacting: bool,
) -> bool {
    if already_compacting || threshold >= 1.0 {
        return false;
    }
    usage.ratio() >= threshold
}

/// Builds a `Vec<ModelMessage>` from `session_events` (preserving tool_call /
/// tool_result id pairing) and trims the FRONT until cumulative input tokens
/// fit `budget_tokens`. The system prompt + the most-recent user message are
/// always kept (they're appended last by `build_model_messages`).
///
/// Token accounting MUST match what providers actually bill — `ModelMessage`
/// has three serialised parts: `role`, `content`, and `tool_calls`
/// (a `Vec<ToolCall>` whose `arguments` is `serde_json::Value`). Tool calls
/// alone often weigh thousands of tokens for non-trivial payloads, so we
/// serialise the whole message to JSON and count that. This matches the
/// estimator used by `ContextAssembler` (cl100k_base on serialised text).
pub fn build_model_messages_within_budget(
    user_content: &str,
    session_events: &[DomainEvent],
    budget_tokens: u64,
) -> Vec<agent_models::ModelMessage> {
    let mut messages = build_model_messages(user_content, session_events);

    let bpe = match tiktoken_rs::cl100k_base() {
        Ok(bpe) => bpe,
        Err(_) => return messages, // tokenizer unavailable; emit as-is
    };
    let count_message = |m: &agent_models::ModelMessage| -> u64 {
        // Use compact JSON to mirror what the OpenAI/Anthropic adapters
        // ultimately serialise. Failures fall back to content-only count.
        match serde_json::to_string(m) {
            Ok(s) => bpe.encode_with_special_tokens(&s).len() as u64,
            Err(_) => bpe.encode_with_special_tokens(&m.content).len() as u64,
        }
    };

    // Always keep the trailing user message (the active turn). Trim from the
    // FRONT, but NEVER drop a `tool` role message without also dropping the
    // matching assistant `tool_calls` message that precedes it — otherwise
    // OpenAI / Anthropic reject the request with "tool_call_id has no
    // matching assistant tool_calls".
    let mut total: u64 = messages.iter().map(&count_message).sum();
    while total > budget_tokens && messages.len() > 1 {
        let front = messages.first().unwrap();
        if front.role == "tool" {
            // No matching assistant left at the front — safe to drop alone.
            total = total.saturating_sub(count_message(front));
            messages.remove(0);
            continue;
        }
        if front.role == "assistant" && !front.tool_calls.is_empty() {
            // Drop the assistant AND every tool message immediately following it
            // (the matching `tool_call_id` results) in one atomic step.
            total = total.saturating_sub(count_message(front));
            messages.remove(0);
            while !messages.is_empty() && messages[0].role == "tool" {
                total = total.saturating_sub(count_message(&messages[0]));
                messages.remove(0);
            }
            continue;
        }
        // Plain user/assistant text — drop one.
        total = total.saturating_sub(count_message(front));
        messages.remove(0);
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AgentId, EventPayload, PrivacyClassification, SessionId, WorkspaceId};

    fn make_event(payload: EventPayload) -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
    }

    #[test]
    fn should_trigger_auto_compaction_uses_threshold_and_not_compacting() {
        let usage_at = |total: u64, budget: u64| -> agent_core::ContextUsage {
            agent_core::ContextUsage {
                total_tokens: total,
                budget_tokens: budget,
                context_window: budget + 12_000,
                output_reservation: 12_000,
                by_source: vec![],
                estimator: "cl100k_base".into(),
                corrected_by_real_usage: false,
            }
        };

        // Below threshold → no trigger.
        assert!(!should_trigger_auto_compaction(
            &usage_at(50_000, 200_000),
            0.85,
            false
        ));
        // At threshold → trigger.
        assert!(should_trigger_auto_compaction(
            &usage_at(170_000, 200_000),
            0.85,
            false
        ));
        // Above threshold but already compacting → no trigger.
        assert!(!should_trigger_auto_compaction(
            &usage_at(190_000, 200_000),
            0.85,
            true
        ));
        // Threshold == 1.0 disables auto-compaction entirely.
        assert!(!should_trigger_auto_compaction(
            &usage_at(199_000, 200_000),
            1.0,
            false
        ));
    }

    #[test]
    fn within_budget_keeps_tail_user_and_pairs_tool_calls() {
        // Build 3 plain user/assistant pairs, each padded so cumulative tokens
        // exceed the 100-token budget and the trimmer must drop from the front.
        let mut events = Vec::new();
        for i in 0..3 {
            events.push(make_event(EventPayload::UserMessageAdded {
                message_id: format!("u{i}"),
                content: format!("user turn {i} ").repeat(20),
            }));
            events.push(make_event(EventPayload::AssistantMessageCompleted {
                message_id: format!("a{i}"),
                content: format!("assistant turn {i} ").repeat(20),
            }));
        }

        let trimmed = build_model_messages_within_budget("latest", &events, 100);

        // (a) total token count <= 100
        let bpe = tiktoken_rs::cl100k_base().unwrap();
        let total: usize = trimmed
            .iter()
            .map(|m| {
                bpe.encode_with_special_tokens(&serde_json::to_string(m).unwrap())
                    .len()
            })
            .sum();
        assert!(total <= 100, "trimmed total {} exceeded budget 100", total);

        // (b) trailing user message is the active turn
        assert_eq!(trimmed.last().map(|m| m.role.as_str()), Some("user"));
        assert_eq!(trimmed.last().map(|m| m.content.as_str()), Some("latest"));

        // (c) every `tool` role message has a preceding assistant with non-empty tool_calls
        for (i, m) in trimmed.iter().enumerate() {
            if m.role == "tool" {
                assert!(i > 0, "tool message at index 0 is unpaired");
                let prev = &trimmed[i - 1];
                assert!(
                    prev.role == "assistant" && !prev.tool_calls.is_empty(),
                    "tool message at {} not preceded by assistant with tool_calls",
                    i
                );
            }
        }
    }
}

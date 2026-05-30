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

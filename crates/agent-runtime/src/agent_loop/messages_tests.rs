use super::*;
use agent_core::{AgentId, CompactionReason, PrivacyClassification, SessionId, WorkspaceId};

#[test]
fn build_model_messages_substitutes_compaction_summary_for_event_range() {
    // Build 5 turns; insert a CompactionSummary covering the first 3 pairs.
    let base = chrono::Utc::now();
    let make_at = |payload: EventPayload, secs: i64| -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
        .with_timestamp(base + chrono::Duration::seconds(secs))
    };

    let mut events: Vec<DomainEvent> = (0..5)
        .flat_map(|i| {
            let t = (i as i64) * 10;
            vec![
                make_at(
                    EventPayload::UserMessageAdded {
                        message_id: format!("u{i}"),
                        content: format!("user {i}"),
                    },
                    t,
                ),
                make_at(
                    EventPayload::AssistantMessageCompleted {
                        message_id: format!("a{i}"),
                        content: format!("assistant {i}"),
                    },
                    t + 1,
                ),
            ]
        })
        .collect();

    let first_ts = events[0].timestamp;
    let last_ts = events[5].timestamp; // covers pairs 0..=2 inclusive
    events.push(make_at(
        EventPayload::CompactionSummary {
            summary_id: "sum_test".into(),
            content: "[SUMMARY] earlier turns about user goal X".into(),
            replaces_event_range: (first_ts, last_ts),
            reason: CompactionReason::UserRequested,
            before_tokens: 1000,
            after_tokens: 50,
            summarised_by_profile: "fast".into(),
        },
        55, // newer than every replaced event but older than the new turn
    ));
    events.sort_by_key(|e| e.timestamp);

    let messages = build_model_messages("latest", &events);

    // (a) The summary text MUST appear in messages.
    let joined: String = messages
        .iter()
        .map(|m| m.content.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        joined.contains("[SUMMARY] earlier turns about user goal X"),
        "summary text missing from assembled messages: {joined}"
    );
    // (b) The replaced "user 0".."assistant 2" content must NOT appear.
    for replaced in [
        "user 0",
        "assistant 0",
        "user 1",
        "assistant 1",
        "user 2",
        "assistant 2",
    ] {
        assert!(
            !joined.contains(replaced),
            "replaced event '{replaced}' leaked into messages: {joined}"
        );
    }
    // (c) The kept tail ("user 3", "assistant 3", "user 4", "assistant 4") must remain.
    for kept in ["user 3", "assistant 3", "user 4", "assistant 4"] {
        assert!(
            joined.contains(kept),
            "kept event '{kept}' missing from messages: {joined}"
        );
    }
    // (d) The trailing "latest" user turn must still be present.
    assert_eq!(messages.last().map(|m| m.content.as_str()), Some("latest"));
}

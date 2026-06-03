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
                        display_content: None,
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

#[test]
fn build_model_messages_replays_tool_use_before_tool_result() {
    let base = chrono::Utc::now();
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let make_at = |payload: EventPayload, secs: i64| -> DomainEvent {
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
        .with_timestamp(base + chrono::Duration::seconds(secs))
    };

    let events = vec![
        make_at(
            EventPayload::UserMessageAdded {
                message_id: "u0".into(),
                content: "read fixture".into(),
                display_content: None,
            },
            0,
        ),
        make_at(
            EventPayload::ModelToolCallRequested {
                tool_call_id: "call_read".into(),
                tool_id: "fs.read".into(),
            },
            1,
        ),
        make_at(
            EventPayload::ToolInvocationCompleted {
                invocation_id: "call_read".into(),
                tool_id: "fs.read".into(),
                output_preview: "KAIROX_PILOT_ATTACHMENT_7F3C9A".into(),
                exit_code: None,
                duration_ms: 4,
                truncated: false,
            },
            2,
        ),
        make_at(
            EventPayload::AssistantMessageCompleted {
                message_id: "a0".into(),
                content: "TOOL-READ-PASS KAIROX_PILOT_ATTACHMENT_7F3C9A".into(),
            },
            3,
        ),
    ];

    let messages = build_model_messages("run pwd next", &events);

    assert_eq!(messages.len(), 5, "{messages:#?}");
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "read fixture");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].tool_calls.len(), 1, "{messages:#?}");
    assert_eq!(messages[1].tool_calls[0].id, "call_read");
    assert_eq!(messages[1].tool_calls[0].name, "fs.read");
    assert_eq!(messages[2].role, "tool");
    assert_eq!(messages[2].tool_call_id.as_deref(), Some("call_read"));
    assert_eq!(messages[3].role, "assistant");
    assert!(messages[3].tool_calls.is_empty(), "{messages:#?}");
    assert_eq!(
        messages[3].content,
        "TOOL-READ-PASS KAIROX_PILOT_ATTACHMENT_7F3C9A"
    );
    assert_eq!(messages[4].role, "user");
    assert_eq!(messages[4].content, "run pwd next");
}

#[test]
fn build_model_messages_keeps_model_content_for_attached_user_messages() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let events = vec![DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::UserMessageAdded {
            message_id: "u-attachment".into(),
            content: "```md\n// file: attachment-fixture.md\nsecret\n```".into(),
            display_content: Some("please inspect @docs/attachment-fixture.md".into()),
        },
    )];

    let messages = build_model_messages(
        "```md\n// file: attachment-fixture.md\nsecret\n```",
        &events,
    );

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "user");
    assert_eq!(
        messages[0].content,
        "```md\n// file: attachment-fixture.md\nsecret\n```"
    );
}

#[test]
fn build_model_messages_replays_permission_denial_as_tool_result() {
    let base = chrono::Utc::now();
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let make_at = |payload: EventPayload, secs: i64| -> DomainEvent {
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
        .with_timestamp(base + chrono::Duration::seconds(secs))
    };

    let events = vec![
        make_at(
            EventPayload::UserMessageAdded {
                message_id: "u0".into(),
                content: "write a file".into(),
                display_content: None,
            },
            0,
        ),
        make_at(
            EventPayload::ModelToolCallRequested {
                tool_call_id: "call_write".into(),
                tool_id: "fs.write".into(),
            },
            1,
        ),
        make_at(
            EventPayload::PermissionDenied {
                request_id: "call_write".into(),
                reason: "read-only sandbox blocks writes".into(),
            },
            2,
        ),
        make_at(
            EventPayload::AssistantMessageCompleted {
                message_id: "a0".into(),
                content: "READONLY-DENIED-PASS".into(),
            },
            3,
        ),
    ];

    let messages = build_model_messages("try workspace write", &events);

    assert_eq!(messages.len(), 5, "{messages:#?}");
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].tool_calls.len(), 1, "{messages:#?}");
    assert_eq!(messages[1].tool_calls[0].id, "call_write");
    assert_eq!(messages[2].role, "tool");
    assert_eq!(messages[2].tool_call_id.as_deref(), Some("call_write"));
    assert!(
        messages[2]
            .content
            .contains("read-only sandbox blocks writes"),
        "{messages:#?}"
    );
    assert_eq!(messages[3].role, "assistant");
    assert!(messages[3].tool_calls.is_empty(), "{messages:#?}");
    assert_eq!(messages[4].role, "user");
    assert_eq!(messages[4].content, "try workspace write");
}

#[test]
fn build_model_messages_closes_cancelled_turn_before_next_user_message() {
    let base = chrono::Utc::now();
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let make_at = |payload: EventPayload, secs: i64| -> DomainEvent {
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
        .with_timestamp(base + chrono::Duration::seconds(secs))
    };

    let events = vec![
        make_at(
            EventPayload::UserMessageAdded {
                message_id: "u0".into(),
                content: "write a very long numbered list".into(),
                display_content: None,
            },
            0,
        ),
        make_at(
            EventPayload::ModelTokenDelta {
                delta: "partial answer".into(),
            },
            1,
        ),
        make_at(
            EventPayload::SessionCancelled {
                reason: "user requested cancellation".into(),
            },
            2,
        ),
        make_at(
            EventPayload::AgentTaskFailed {
                task_id: agent_core::TaskId::new(),
                error: "cancelled by user".into(),
            },
            3,
        ),
        make_at(
            EventPayload::UserMessageAdded {
                message_id: "u1".into(),
                content: "AFTER-CANCEL-OK".into(),
                display_content: None,
            },
            4,
        ),
    ];

    let messages = build_model_messages("AFTER-CANCEL-OK", &events);

    assert_eq!(
        messages
            .iter()
            .map(|message| message.role.as_str())
            .collect::<Vec<_>>(),
        vec!["user", "assistant", "user"],
        "{messages:#?}"
    );
    assert_eq!(messages[0].content, "write a very long numbered list");
    assert!(
        messages[1].content.contains("cancelled by the user"),
        "{messages:#?}"
    );
    assert_eq!(messages[2].content, "AFTER-CANCEL-OK");
}

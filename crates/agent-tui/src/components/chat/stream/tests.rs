use super::*;
use agent_core::events::{CompactionReason, MonitorStopReason};
use agent_core::TaskConfirmationOption;
use agent_core::{AgentId, DomainEvent, PrivacyClassification, SessionId, WorkspaceId};
use chrono::{Duration as ChronoDuration, TimeZone, Utc};

fn make_event_at(offset_ms: i64, payload: EventPayload) -> DomainEvent {
    let timestamp = Utc.timestamp_opt(0, 0).unwrap() + ChronoDuration::milliseconds(offset_ms);
    DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        payload,
    )
    .with_timestamp(timestamp)
}

#[test]
fn compaction_item_carries_before_and_after_tokens() {
    let events = vec![
        make_event_at(
            100,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 25_000,
                candidate_event_count: 12,
            },
        ),
        make_event_at(
            900,
            EventPayload::ContextCompactionCompleted {
                summary_id: "sum_x".into(),
                after_tokens: 12_000,
                fallback_used: false,
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    let compactions: Vec<&ChatStreamItem> = items
        .iter()
        .filter(|item| matches!(item, ChatStreamItem::Compaction { .. }))
        .collect();
    assert_eq!(
        compactions.len(),
        1,
        "expected exactly one Compaction stream item, got {items:?}"
    );
    match compactions[0] {
        ChatStreamItem::Compaction {
            status,
            before_tokens,
            after_tokens,
            ..
        } => {
            assert_eq!(*status, CompactionItemStatus::Completed);
            assert_eq!(
                *before_tokens,
                Some(25_000),
                "before_tokens should be lifted from ContextCompactionStarted"
            );
            assert_eq!(
                *after_tokens,
                Some(12_000),
                "after_tokens should be lifted from ContextCompactionCompleted"
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn monitor_started_creates_running_item() {
    let events = vec![make_event_at(
        100,
        EventPayload::MonitorStarted {
            monitor_id: "mon_1".into(),
            description: "watch build".into(),
            command: "tail -f build.log".into(),
            persistent: false,
            timeout_ms: 300_000,
        },
    )];
    let projection = SessionProjection::from_events(&events);
    let items = fold_stream(&projection, &events);

    let monitors: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, ChatStreamItem::Monitor { .. }))
        .collect();
    assert_eq!(monitors.len(), 1);
    match &monitors[0] {
        ChatStreamItem::Monitor {
            monitor_id,
            description,
            status,
            last_line,
            ..
        } => {
            assert_eq!(monitor_id, "mon_1");
            assert_eq!(description, "watch build");
            assert_eq!(*status, MonitorItemStatus::Running);
            assert!(last_line.is_none());
        }
        _ => unreachable!(),
    }
}

#[test]
fn task_confirmation_requested_and_resolved_updates_stream_item() {
    let events = vec![
        make_event_at(
            100,
            EventPayload::TaskConfirmationRequested {
                request_id: "confirm-1".into(),
                prompt: "Choose path".into(),
                options: vec![
                    TaskConfirmationOption {
                        id: "small".into(),
                        label: "Small fix".into(),
                        description: Some("one module".into()),
                    },
                    TaskConfirmationOption {
                        id: "broad".into(),
                        label: "Broad pass".into(),
                        description: None,
                    },
                ],
                allow_multiple: true,
                allow_custom: true,
            },
        ),
        make_event_at(
            200,
            EventPayload::TaskConfirmationResolved {
                request_id: "confirm-1".into(),
                selected_option_ids: vec!["small".into()],
                custom_response: Some("keep API stable".into()),
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);
    let items = fold_stream(&projection, &events);

    let confirmations: Vec<_> = items
        .iter()
        .filter(|item| matches!(item, ChatStreamItem::TaskConfirmation { .. }))
        .collect();
    assert_eq!(confirmations.len(), 1);
    match confirmations[0] {
        ChatStreamItem::TaskConfirmation {
            id,
            prompt,
            options,
            allow_multiple,
            allow_custom,
            status,
            selected_option_ids,
            custom_response,
            ..
        } => {
            assert_eq!(id, "confirm-1");
            assert_eq!(prompt, "Choose path");
            assert_eq!(options[0].id, "small");
            assert!(*allow_multiple);
            assert!(*allow_custom);
            assert_eq!(*status, TaskConfirmationStatus::Resolved);
            assert_eq!(selected_option_ids, &vec!["small".to_string()]);
            assert_eq!(custom_response.as_deref(), Some("keep API stable"));
        }
        _ => unreachable!(),
    }
}

#[test]
fn monitor_event_updates_last_line() {
    let events = vec![
        make_event_at(
            100,
            EventPayload::MonitorStarted {
                monitor_id: "mon_1".into(),
                description: "watch".into(),
                command: "cmd".into(),
                persistent: false,
                timeout_ms: 300_000,
            },
        ),
        make_event_at(
            200,
            EventPayload::MonitorEvent {
                monitor_id: "mon_1".into(),
                line: "first line".into(),
            },
        ),
        make_event_at(
            300,
            EventPayload::MonitorEvent {
                monitor_id: "mon_1".into(),
                line: "second line".into(),
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);
    let items = fold_stream(&projection, &events);

    let mon = items
        .iter()
        .find(|i| matches!(i, ChatStreamItem::Monitor { .. }))
        .unwrap();
    match mon {
        ChatStreamItem::Monitor { last_line, .. } => {
            assert_eq!(last_line.as_deref(), Some("second line"));
        }
        _ => unreachable!(),
    }
}

#[test]
fn monitor_stopped_updates_status() {
    let events = vec![
        make_event_at(
            100,
            EventPayload::MonitorStarted {
                monitor_id: "mon_1".into(),
                description: "watch".into(),
                command: "cmd".into(),
                persistent: false,
                timeout_ms: 300_000,
            },
        ),
        make_event_at(
            500,
            EventPayload::MonitorStopped {
                monitor_id: "mon_1".into(),
                reason: MonitorStopReason::ExitCode { code: 0 },
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);
    let items = fold_stream(&projection, &events);

    let mon = items
        .iter()
        .find(|i| matches!(i, ChatStreamItem::Monitor { .. }))
        .unwrap();
    match mon {
        ChatStreamItem::Monitor { status, .. } => {
            assert_eq!(
                *status,
                MonitorItemStatus::Stopped(MonitorStopReason::ExitCode { code: 0 })
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn monitor_failed_sets_error_as_last_line() {
    let events = vec![
        make_event_at(
            100,
            EventPayload::MonitorStarted {
                monitor_id: "mon_1".into(),
                description: "watch".into(),
                command: "cmd".into(),
                persistent: false,
                timeout_ms: 300_000,
            },
        ),
        make_event_at(
            200,
            EventPayload::MonitorFailed {
                monitor_id: "mon_1".into(),
                error: "spawn failed".into(),
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);
    let items = fold_stream(&projection, &events);

    let mon = items
        .iter()
        .find(|i| matches!(i, ChatStreamItem::Monitor { .. }))
        .unwrap();
    match mon {
        ChatStreamItem::Monitor {
            status, last_line, ..
        } => {
            assert_eq!(*status, MonitorItemStatus::Failed);
            assert_eq!(last_line.as_deref(), Some("spawn failed"));
        }
        _ => unreachable!(),
    }
}

#[test]
fn monitor_item_id_and_timestamp_accessors() {
    let events = vec![make_event_at(
        42,
        EventPayload::MonitorStarted {
            monitor_id: "mon_x".into(),
            description: "d".into(),
            command: "c".into(),
            persistent: false,
            timeout_ms: 0,
        },
    )];
    let projection = SessionProjection::from_events(&events);
    let items = fold_stream(&projection, &events);
    let mon = &items[0];
    assert_eq!(mon.id(), "monitor-mon_x");
    assert_eq!(mon.timestamp_ms(), 42);
}

#[test]
fn failed_compaction_leaves_after_tokens_none() {
    let events = vec![
        make_event_at(
            100,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 25_000,
                candidate_event_count: 12,
            },
        ),
        make_event_at(
            900,
            EventPayload::ContextCompactionFailed {
                error: "model timeout".into(),
                fallback_used: false,
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);
    let compaction = items
        .iter()
        .find(|item| matches!(item, ChatStreamItem::Compaction { .. }))
        .expect("expected one Compaction item");
    match compaction {
        ChatStreamItem::Compaction {
            status,
            before_tokens,
            after_tokens,
            ..
        } => {
            assert_eq!(*status, CompactionItemStatus::Failed);
            assert_eq!(*before_tokens, Some(25_000));
            assert_eq!(
                *after_tokens, None,
                "failed compactions do not carry an after_tokens value"
            );
        }
        _ => unreachable!(),
    }
}

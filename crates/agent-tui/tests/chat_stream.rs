//! Integration tests for the TUI `ChatStreamItem` reducer.
//!
//! Mirrors the GUI `useChatStream` composable (see
//! `apps/agent-gui/src/composables/useChatStream.ts`): a pure fold that
//! walks the session's domain events and emits a single chronologically
//! ordered list of items to render inline in `ChatPanel`. This file is
//! the foundational test contract for the reducer — the renderer port
//! lands in a follow-up PR.

use agent_core::events::{CompactionReason, EventPayload, MonitorStopReason};
use agent_core::projection::SessionProjection;
use agent_core::{AgentId, DomainEvent, PrivacyClassification, SessionId, WorkspaceId};
use chrono::{Duration as ChronoDuration, TimeZone, Utc};

use agent_tui::components::chat::stream::{
    fold_stream, ChatStreamItem, CompactionItemStatus, MessageRole, MonitorItemStatus,
    PermissionKind, PermissionStatus, ToolCallStatus,
};

fn make_event(payload: EventPayload) -> DomainEvent {
    DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        payload,
    )
}

/// Build an event whose timestamp is `epoch + offset_ms` so tests can
/// pin chronological ordering without sleeping.
fn make_event_at(offset_ms: i64, payload: EventPayload) -> DomainEvent {
    let timestamp = Utc.timestamp_opt(0, 0).unwrap() + ChronoDuration::milliseconds(offset_ms);
    make_event(payload).with_timestamp(timestamp)
}

#[test]
fn folds_message_events_in_chronological_order() {
    let events = vec![
        make_event_at(
            10,
            EventPayload::UserMessageAdded {
                message_id: "u1".into(),
                content: "hello".into(),
                display_content: None,
            },
        ),
        make_event_at(
            20,
            EventPayload::AssistantMessageCompleted {
                message_id: "a1".into(),
                content: "hi back".into(),
            },
        ),
        make_event_at(
            30,
            EventPayload::UserMessageAdded {
                message_id: "u2".into(),
                content: "follow-up".into(),
                display_content: None,
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    assert_eq!(items.len(), 3, "expected one stream item per message event");
    match &items[0] {
        ChatStreamItem::Message {
            id,
            role,
            content,
            timestamp_ms,
        } => {
            assert_eq!(id, "u1");
            assert_eq!(*role, MessageRole::User);
            assert_eq!(content, "hello");
            assert_eq!(*timestamp_ms, 10);
        }
        other => panic!("expected Message for events[0], got {other:?}"),
    }
    match &items[1] {
        ChatStreamItem::Message {
            id,
            role,
            content,
            timestamp_ms,
        } => {
            assert_eq!(id, "a1");
            assert_eq!(*role, MessageRole::Assistant);
            assert_eq!(content, "hi back");
            assert_eq!(*timestamp_ms, 20);
        }
        other => panic!("expected Message for events[1], got {other:?}"),
    }
    match &items[2] {
        ChatStreamItem::Message {
            id,
            role,
            timestamp_ms,
            ..
        } => {
            assert_eq!(id, "u2");
            assert_eq!(*role, MessageRole::User);
            assert_eq!(*timestamp_ms, 30);
        }
        other => panic!("expected Message for events[2], got {other:?}"),
    }
}

#[test]
fn folds_tool_call_lifecycle_into_single_completed_item() {
    let events = vec![
        make_event_at(
            100,
            EventPayload::ModelToolCallRequested {
                tool_call_id: "call_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            110,
            EventPayload::ToolInvocationStarted {
                invocation_id: "inv_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            220,
            EventPayload::ToolInvocationCompleted {
                invocation_id: "inv_1".into(),
                tool_id: "shell.exec".into(),
                output_preview: "ok\nline 2".into(),
                exit_code: Some(0),
                duration_ms: 120,
                truncated: false,
                images: vec![],
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    let tool_items: Vec<&ChatStreamItem> = items
        .iter()
        .filter(|item| matches!(item, ChatStreamItem::ToolCall { .. }))
        .collect();
    assert_eq!(
        tool_items.len(),
        1,
        "tool call lifecycle should collapse into one item, got {items:?}"
    );
    match tool_items[0] {
        ChatStreamItem::ToolCall {
            tool_id,
            status,
            output_preview,
            duration_ms,
            timestamp_ms,
            ..
        } => {
            assert_eq!(tool_id, "shell.exec");
            assert_eq!(*status, ToolCallStatus::Completed);
            assert_eq!(output_preview.as_deref(), Some("ok\nline 2"));
            assert_eq!(*duration_ms, Some(120));
            assert_eq!(
                *timestamp_ms, 100,
                "tool call item should carry the timestamp of the first \
                 lifecycle event (ModelToolCallRequested)"
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn permission_with_no_resolution_stays_pending() {
    let events = vec![make_event_at(
        50,
        EventPayload::PermissionRequested {
            request_id: "req_1".into(),
            tool_id: "shell.exec".into(),
            preview: "rm -rf /tmp/foo".into(),
        },
    )];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    assert_eq!(items.len(), 1);
    match &items[0] {
        ChatStreamItem::Permission {
            id,
            kind,
            prompt,
            status,
            timestamp_ms,
        } => {
            assert_eq!(id, "req_1");
            assert_eq!(*kind, PermissionKind::Tool);
            assert_eq!(prompt, "rm -rf /tmp/foo");
            assert_eq!(*status, PermissionStatus::Pending);
            assert_eq!(*timestamp_ms, 50);
        }
        other => panic!("expected Permission, got {other:?}"),
    }
}

#[test]
fn permission_resolved_into_accepted_status() {
    let events = vec![
        make_event_at(
            50,
            EventPayload::PermissionRequested {
                request_id: "req_1".into(),
                tool_id: "shell.exec".into(),
                preview: "rm -rf /tmp/foo".into(),
            },
        ),
        make_event_at(
            60,
            EventPayload::PermissionGranted {
                request_id: "req_1".into(),
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    let perms: Vec<&ChatStreamItem> = items
        .iter()
        .filter(|item| matches!(item, ChatStreamItem::Permission { .. }))
        .collect();
    assert_eq!(
        perms.len(),
        1,
        "permission lifecycle should collapse into one item — reducer \
         keeps resolved items so the renderer can filter, got {items:?}"
    );
    match perms[0] {
        ChatStreamItem::Permission {
            id,
            status,
            timestamp_ms,
            ..
        } => {
            assert_eq!(id, "req_1");
            assert_eq!(*status, PermissionStatus::Accepted);
            assert_eq!(
                *timestamp_ms, 50,
                "permission item should keep the timestamp of the request \
                 event so chronological ordering is stable across resolution"
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn compaction_started_then_completed_with_summary() {
    let events = vec![
        make_event_at(
            1_000,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 180_000,
                candidate_event_count: 42,
            },
        ),
        make_event_at(
            2_500,
            EventPayload::ContextCompactionCompleted {
                summary_id: "sum_1".into(),
                after_tokens: 30_000,
                fallback_used: false,
            },
        ),
        make_event_at(
            2_600,
            EventPayload::CompactionSummary {
                summary_id: "sum_1".into(),
                content: "Earlier turns summarised to free up budget.".into(),
                replaces_event_range: (
                    Utc.timestamp_opt(0, 0).unwrap(),
                    Utc.timestamp_opt(1, 0).unwrap(),
                ),
                reason: CompactionReason::UserRequested,
                before_tokens: 180_000,
                after_tokens: 30_000,
                summarised_by_profile: "gpt-5".into(),
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
        "compaction lifecycle should collapse into one item, got {items:?}"
    );
    match compactions[0] {
        ChatStreamItem::Compaction {
            status,
            summary,
            timestamp_ms,
            ..
        } => {
            assert_eq!(*status, CompactionItemStatus::Completed);
            assert_eq!(
                summary.as_deref(),
                Some("Earlier turns summarised to free up budget."),
                "CompactionSummary content should be folded into the \
                 compaction item once it arrives"
            );
            assert_eq!(
                *timestamp_ms, 1_000,
                "compaction item should keep the timestamp of the start event"
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn chronological_interleaving_across_item_kinds() {
    let events = vec![
        make_event_at(
            10,
            EventPayload::UserMessageAdded {
                message_id: "u1".into(),
                content: "run a shell command".into(),
                display_content: None,
            },
        ),
        make_event_at(
            20,
            EventPayload::PermissionRequested {
                request_id: "req_1".into(),
                tool_id: "shell.exec".into(),
                preview: "ls".into(),
            },
        ),
        make_event_at(
            30,
            EventPayload::PermissionGranted {
                request_id: "req_1".into(),
            },
        ),
        make_event_at(
            40,
            EventPayload::ModelToolCallRequested {
                tool_call_id: "call_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            45,
            EventPayload::ToolInvocationStarted {
                invocation_id: "inv_1".into(),
                tool_id: "shell.exec".into(),
            },
        ),
        make_event_at(
            55,
            EventPayload::ToolInvocationCompleted {
                invocation_id: "inv_1".into(),
                tool_id: "shell.exec".into(),
                output_preview: "ok".into(),
                exit_code: Some(0),
                duration_ms: 10,
                truncated: false,
                images: vec![],
            },
        ),
        make_event_at(
            70,
            EventPayload::AssistantMessageCompleted {
                message_id: "a1".into(),
                content: "done".into(),
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    let timestamps: Vec<i64> = items.iter().map(ChatStreamItem::timestamp_ms).collect();
    let mut sorted = timestamps.clone();
    sorted.sort();
    assert_eq!(
        timestamps, sorted,
        "items must be emitted in chronological order, got {items:?}"
    );

    // Also assert the expected kind sequence: message, permission, tool call, message.
    let kinds: Vec<&'static str> = items
        .iter()
        .map(|item| match item {
            ChatStreamItem::Message { .. } => "message",
            ChatStreamItem::ToolCall { .. } => "tool_call",
            ChatStreamItem::Permission { .. } => "permission",
            ChatStreamItem::TaskConfirmation { .. } => "task_confirmation",
            ChatStreamItem::Compaction { .. } => "compaction",
            ChatStreamItem::CompactionSkipped { .. } => "compaction_skipped",
            ChatStreamItem::Monitor { .. } => "monitor",
        })
        .collect();
    assert_eq!(
        kinds,
        vec!["message", "permission", "tool_call", "message"],
        "expected chronological kind sequence; got {kinds:?} from items {items:?}"
    );
}

// ── Monitor fold tests ──────────────────────────────────────────────

#[test]
fn folds_monitor_started_into_running_item() {
    let events = vec![make_event_at(
        10,
        EventPayload::MonitorStarted {
            monitor_id: "mon_1".into(),
            description: "watch build".into(),
            command: "tail -f build.log".into(),
            persistent: false,
            timeout_ms: 5_000,
        },
    )];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    assert_eq!(items.len(), 1);
    match &items[0] {
        ChatStreamItem::Monitor {
            id,
            monitor_id,
            description,
            status,
            last_line,
            timestamp_ms,
        } => {
            assert_eq!(id, "monitor-mon_1");
            assert_eq!(monitor_id, "mon_1");
            assert_eq!(description, "watch build");
            assert_eq!(*status, MonitorItemStatus::Running);
            assert!(last_line.is_none());
            assert_eq!(*timestamp_ms, 10);
        }
        other => panic!("expected Monitor, got {other:?}"),
    }
}

#[test]
fn monitor_event_updates_last_line() {
    let events = vec![
        make_event_at(
            10,
            EventPayload::MonitorStarted {
                monitor_id: "mon_1".into(),
                description: "watch".into(),
                command: "cmd".into(),
                persistent: false,
                timeout_ms: 5_000,
            },
        ),
        make_event_at(
            20,
            EventPayload::MonitorEvent {
                monitor_id: "mon_1".into(),
                line: "first line".into(),
            },
        ),
        make_event_at(
            30,
            EventPayload::MonitorEvent {
                monitor_id: "mon_1".into(),
                line: "second line".into(),
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    assert_eq!(items.len(), 1);
    if let ChatStreamItem::Monitor {
        last_line, status, ..
    } = &items[0]
    {
        assert_eq!(*status, MonitorItemStatus::Running);
        assert_eq!(last_line.as_deref(), Some("second line"));
    } else {
        panic!("expected Monitor");
    }
}

#[test]
fn monitor_stopped_updates_status() {
    let events = vec![
        make_event_at(
            10,
            EventPayload::MonitorStarted {
                monitor_id: "mon_1".into(),
                description: "watch".into(),
                command: "cmd".into(),
                persistent: false,
                timeout_ms: 5_000,
            },
        ),
        make_event_at(
            20,
            EventPayload::MonitorStopped {
                monitor_id: "mon_1".into(),
                reason: MonitorStopReason::ExitCode { code: 0 },
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    assert_eq!(items.len(), 1);
    if let ChatStreamItem::Monitor { status, .. } = &items[0] {
        assert_eq!(
            *status,
            MonitorItemStatus::Stopped(MonitorStopReason::ExitCode { code: 0 })
        );
    } else {
        panic!("expected Monitor");
    }
}

#[test]
fn monitor_failed_sets_error_as_last_line() {
    let events = vec![
        make_event_at(
            10,
            EventPayload::MonitorStarted {
                monitor_id: "mon_1".into(),
                description: "watch".into(),
                command: "cmd".into(),
                persistent: false,
                timeout_ms: 5_000,
            },
        ),
        make_event_at(
            20,
            EventPayload::MonitorFailed {
                monitor_id: "mon_1".into(),
                error: "spawn failed".into(),
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    assert_eq!(items.len(), 1);
    if let ChatStreamItem::Monitor {
        status, last_line, ..
    } = &items[0]
    {
        assert_eq!(*status, MonitorItemStatus::Failed);
        assert_eq!(last_line.as_deref(), Some("spawn failed"));
    } else {
        panic!("expected Monitor");
    }
}

#[test]
fn multiple_monitors_fold_independently() {
    let events = vec![
        make_event_at(
            10,
            EventPayload::MonitorStarted {
                monitor_id: "mon_1".into(),
                description: "watch A".into(),
                command: "cmd-a".into(),
                persistent: false,
                timeout_ms: 5_000,
            },
        ),
        make_event_at(
            20,
            EventPayload::MonitorStarted {
                monitor_id: "mon_2".into(),
                description: "watch B".into(),
                command: "cmd-b".into(),
                persistent: true,
                timeout_ms: 60_000,
            },
        ),
        make_event_at(
            30,
            EventPayload::MonitorEvent {
                monitor_id: "mon_1".into(),
                line: "output-a".into(),
            },
        ),
        make_event_at(
            40,
            EventPayload::MonitorStopped {
                monitor_id: "mon_1".into(),
                reason: MonitorStopReason::Timeout,
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    assert_eq!(items.len(), 2);
    if let ChatStreamItem::Monitor {
        monitor_id,
        status,
        last_line,
        ..
    } = &items[0]
    {
        assert_eq!(monitor_id, "mon_1");
        assert_eq!(
            *status,
            MonitorItemStatus::Stopped(MonitorStopReason::Timeout)
        );
        assert_eq!(last_line.as_deref(), Some("output-a"));
    } else {
        panic!("expected Monitor for items[0]");
    }
    if let ChatStreamItem::Monitor {
        monitor_id, status, ..
    } = &items[1]
    {
        assert_eq!(monitor_id, "mon_2");
        assert_eq!(*status, MonitorItemStatus::Running);
    } else {
        panic!("expected Monitor for items[1]");
    }
}

#[test]
fn monitor_interleaves_with_messages_chronologically() {
    let events = vec![
        make_event_at(
            10,
            EventPayload::UserMessageAdded {
                message_id: "u1".into(),
                content: "start monitoring".into(),
                display_content: None,
            },
        ),
        make_event_at(
            20,
            EventPayload::MonitorStarted {
                monitor_id: "mon_1".into(),
                description: "watch".into(),
                command: "cmd".into(),
                persistent: false,
                timeout_ms: 5_000,
            },
        ),
        make_event_at(
            30,
            EventPayload::AssistantMessageCompleted {
                message_id: "a1".into(),
                content: "monitoring started".into(),
            },
        ),
    ];
    let projection = SessionProjection::from_events(&events);

    let items = fold_stream(&projection, &events);

    assert_eq!(items.len(), 3);
    let kinds: Vec<&str> = items
        .iter()
        .map(|item| match item {
            ChatStreamItem::Message { .. } => "message",
            ChatStreamItem::Monitor { .. } => "monitor",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, vec!["message", "monitor", "message"]);
}

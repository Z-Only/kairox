//! Integration test: every EventPayload variant round-trips through JSON serde.

use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, TaskId, WorkspaceId,
};
use chrono::TimeZone;

fn make_event(payload: EventPayload) -> DomainEvent {
    DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        payload,
    )
}

fn roundtrip(event: &DomainEvent) -> DomainEvent {
    let json = serde_json::to_string(event).unwrap();
    serde_json::from_str(&json).unwrap()
}

#[test]
fn workspace_opened_roundtrips() {
    let event = make_event(EventPayload::WorkspaceOpened {
        path: "/tmp/project".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn user_message_added_roundtrips() {
    let event = make_event(EventPayload::UserMessageAdded {
        message_id: "m1".into(),
        content: "hello world".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn agent_task_created_roundtrips() {
    let event = make_event(EventPayload::AgentTaskCreated {
        task_id: TaskId::new(),
        title: "inspect repo".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn agent_task_started_roundtrips() {
    let event = make_event(EventPayload::AgentTaskStarted {
        task_id: TaskId::new(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn context_assembled_roundtrips() {
    let event = make_event(EventPayload::ContextAssembled {
        token_estimate: 4096,
        sources: vec!["memory".into(), "system".into()],
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn model_request_started_roundtrips() {
    let event = make_event(EventPayload::ModelRequestStarted {
        model_profile: "fast".into(),
        model_id: "gpt-4.1-mini".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn model_token_delta_roundtrips() {
    let event = make_event(EventPayload::ModelTokenDelta {
        delta: "hello".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn model_tool_call_requested_roundtrips() {
    let event = make_event(EventPayload::ModelToolCallRequested {
        tool_call_id: "call_1".into(),
        tool_id: "shell.exec".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn permission_requested_roundtrips() {
    let event = make_event(EventPayload::PermissionRequested {
        request_id: "req_1".into(),
        tool_id: "shell.exec".into(),
        preview: "rm -rf /tmp/test".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn permission_granted_roundtrips() {
    let event = make_event(EventPayload::PermissionGranted {
        request_id: "req_1".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn permission_denied_roundtrips() {
    let event = make_event(EventPayload::PermissionDenied {
        request_id: "req_1".into(),
        reason: "destructive operation".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn tool_invocation_started_roundtrips() {
    let event = make_event(EventPayload::ToolInvocationStarted {
        invocation_id: "inv_1".into(),
        tool_id: "shell.exec".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn tool_invocation_completed_roundtrips() {
    let event = make_event(EventPayload::ToolInvocationCompleted {
        invocation_id: "inv_1".into(),
        tool_id: "shell.exec".into(),
        output_preview: "file.txt".into(),
        exit_code: Some(0),
        duration_ms: 150,
        truncated: false,
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn tool_invocation_failed_roundtrips() {
    let event = make_event(EventPayload::ToolInvocationFailed {
        invocation_id: "inv_1".into(),
        tool_id: "shell.exec".into(),
        error: "command not found".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn file_patch_proposed_roundtrips() {
    let event = make_event(EventPayload::FilePatchProposed {
        patch_id: "p1".into(),
        diff: "--- a/foo.rs\n+++ b/foo.rs\n".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn file_patch_applied_roundtrips() {
    let event = make_event(EventPayload::FilePatchApplied {
        patch_id: "p1".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn memory_proposed_roundtrips() {
    let event = make_event(EventPayload::MemoryProposed {
        memory_id: "mem_1".into(),
        scope: "workspace".into(),
        key: Some("build-cmd".into()),
        content: "cargo nextest".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn memory_accepted_roundtrips() {
    let event = make_event(EventPayload::MemoryAccepted {
        memory_id: "mem_1".into(),
        scope: "user".into(),
        key: Some("preferred-language".into()),
        content: "Rust".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn memory_rejected_roundtrips() {
    let event = make_event(EventPayload::MemoryRejected {
        memory_id: "mem_1".into(),
        reason: "inaccurate".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn reviewer_finding_added_roundtrips() {
    let event = make_event(EventPayload::ReviewerFindingAdded {
        finding_id: "f1".into(),
        severity: "high".into(),
        message: "destructive command detected".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn assistant_message_completed_roundtrips() {
    let event = make_event(EventPayload::AssistantMessageCompleted {
        message_id: "m2".into(),
        content: "Here's the answer.".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn agent_task_completed_roundtrips() {
    let event = make_event(EventPayload::AgentTaskCompleted {
        task_id: TaskId::new(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn agent_task_failed_roundtrips() {
    let event = make_event(EventPayload::AgentTaskFailed {
        task_id: TaskId::new(),
        error: "timeout".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn session_cancelled_roundtrips() {
    let event = make_event(EventPayload::SessionCancelled {
        reason: "user stopped".into(),
    });
    assert_eq!(roundtrip(&event), event);
}

#[test]
fn event_with_fixed_timestamp_roundtrips() {
    let event = make_event(EventPayload::UserMessageAdded {
        message_id: "m1".into(),
        content: "hello".into(),
    })
    .with_timestamp(
        chrono::Utc
            .with_ymd_and_hms(2026, 1, 15, 10, 30, 0)
            .unwrap(),
    );
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("2026-01-15T10:30:00Z"));
    let rt: DomainEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(rt.timestamp, event.timestamp);
    assert_eq!(rt.payload, event.payload);
}

use super::*;
use crate::{AgentId, AgentRole, EventPayload, PrivacyClassification, TaskId};

#[test]
fn task_snapshot_field_access() {
    let snapshot = TaskSnapshot {
        id: TaskId::new(),
        title: "review PR #42".into(),
        role: AgentRole::Reviewer,
        state: TaskState::Pending,
        dependencies: vec![],
        error: None,
        retry_count: 0,
        max_retries: 3,
        assigned_agent_id: None,
        failure_reason: None,
    };

    // Verify fields hold the values we set.
    assert_eq!(snapshot.title, "review PR #42");
    assert_eq!(snapshot.role, AgentRole::Reviewer);
    assert_eq!(snapshot.state, TaskState::Pending);
    assert!(snapshot.dependencies.is_empty());
    assert!(snapshot.error.is_none());
    assert_eq!(snapshot.retry_count, 0);
    assert_eq!(snapshot.max_retries, 3);
    assert!(snapshot.assigned_agent_id.is_none());
    assert!(snapshot.failure_reason.is_none());
}

#[test]
fn task_snapshot_with_error_and_failure_reason() {
    let failure = TaskFailureReason::ToolExhausted {
        tool_id: "shell.exec".into(),
        attempts: 3,
        last_error: "command not found".into(),
    };
    let snapshot = TaskSnapshot {
        id: TaskId::new(),
        title: "run tests".into(),
        role: AgentRole::Worker,
        state: TaskState::Failed,
        dependencies: vec![],
        error: Some("max retries exceeded".into()),
        retry_count: 3,
        max_retries: 3,
        assigned_agent_id: Some("agent_worker_test".into()),
        failure_reason: Some(failure.clone()),
    };

    assert_eq!(snapshot.state, TaskState::Failed);
    assert_eq!(snapshot.error.as_deref(), Some("max retries exceeded"));
    assert_eq!(snapshot.retry_count, 3);
    assert_eq!(
        snapshot.assigned_agent_id.as_deref(),
        Some("agent_worker_test")
    );
    assert_eq!(snapshot.failure_reason, Some(failure));
}

#[test]
fn task_graph_snapshot_contains_tasks() {
    let task1 = TaskSnapshot {
        id: TaskId::new(),
        title: "plan architecture".into(),
        role: AgentRole::Planner,
        state: TaskState::Completed,
        dependencies: vec![],
        error: None,
        retry_count: 0,
        max_retries: 3,
        assigned_agent_id: Some("agent_planner".into()),
        failure_reason: None,
    };
    let task2 = TaskSnapshot {
        id: TaskId::new(),
        title: "implement feature".into(),
        role: AgentRole::Worker,
        state: TaskState::Running,
        dependencies: vec![task1.id.clone()],
        error: None,
        retry_count: 0,
        max_retries: 3,
        assigned_agent_id: Some("agent_worker_impl".into()),
        failure_reason: None,
    };

    let graph = TaskGraphSnapshot {
        tasks: vec![task1.clone(), task2.clone()],
    };

    assert_eq!(graph.tasks.len(), 2);
    assert!(graph.tasks.contains(&task1));
    assert!(graph.tasks.contains(&task2));

    // task2 depends on task1.
    assert_eq!(graph.tasks[1].dependencies, vec![task1.id.clone()]);
}

#[test]
fn task_graph_snapshot_serializes_roundtrip() {
    let task = TaskSnapshot {
        id: TaskId::new(),
        title: "verify".into(),
        role: AgentRole::Reviewer,
        state: TaskState::Completed,
        dependencies: vec![],
        error: None,
        retry_count: 0,
        max_retries: 3,
        assigned_agent_id: None,
        failure_reason: None,
    };
    let graph = TaskGraphSnapshot { tasks: vec![task] };

    let json = serde_json::to_string(&graph).unwrap();
    let back: TaskGraphSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(graph, back);
}

#[test]
fn trace_export_envelope_counts_events() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let events = vec![
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::SessionInitialized {
                model_profile: "fake".into(),
            },
        ),
        DomainEvent::new(
            workspace_id,
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::UserMessageAdded {
                message_id: "msg-1".into(),
                content: "hello".into(),
            },
        ),
    ];

    let export = TraceExport::new(session_id.clone(), events.clone());

    assert_eq!(export.schema_version, 1);
    assert_eq!(export.session_id, session_id);
    assert_eq!(export.event_count, 2);
    assert_eq!(export.events, events);
}

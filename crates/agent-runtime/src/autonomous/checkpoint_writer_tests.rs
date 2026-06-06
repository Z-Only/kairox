use agent_core::autonomous::{AutonomousTaskGoal, SessionEndReason};
use agent_core::{
    AgentId, AgentRole, DomainEvent, EventPayload, PrivacyClassification, SessionId, TaskId,
    WorkspaceId,
};

use super::*;

fn make_event(payload: EventPayload) -> DomainEvent {
    DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        payload,
    )
}

fn sample_goal() -> AutonomousTaskGoal {
    AutonomousTaskGoal {
        description: "Build feature X".into(),
        acceptance_criteria: vec![
            "tests pass".into(),
            "no lint errors".into(),
            "docs updated".into(),
        ],
        verification_commands: vec!["cargo test".into()],
    }
}

#[test]
fn detect_end_reason_task_completed() {
    let events = vec![
        make_event(EventPayload::AgentTaskCreated {
            task_id: TaskId::new(),
            title: "root".into(),
            role: AgentRole::Planner,
            dependencies: vec![],
        }),
        make_event(EventPayload::AgentTaskCompleted {
            task_id: TaskId::new(),
        }),
    ];
    assert_eq!(
        CheckpointWriter::detect_end_reason(&events),
        SessionEndReason::TaskCompleted
    );
}

#[test]
fn detect_end_reason_cancelled() {
    let events = vec![make_event(EventPayload::SessionCancelled {
        reason: "user".into(),
    })];
    assert_eq!(
        CheckpointWriter::detect_end_reason(&events),
        SessionEndReason::UserPaused
    );
}

#[test]
fn detect_end_reason_max_iterations() {
    let events = vec![make_event(EventPayload::AgentTaskFailed {
        task_id: TaskId::new(),
        error: "max iterations exceeded".into(),
    })];
    assert_eq!(
        CheckpointWriter::detect_end_reason(&events),
        SessionEndReason::MaxIterationsReached
    );
}

#[test]
fn detect_end_reason_task_failed() {
    let events = vec![make_event(EventPayload::AgentTaskFailed {
        task_id: TaskId::new(),
        error: "model error".into(),
    })];
    assert_eq!(
        CheckpointWriter::detect_end_reason(&events),
        SessionEndReason::TaskFailed
    );
}

#[test]
fn build_checkpoint_extracts_completed_tasks() {
    let task_id = TaskId::new();
    let events = vec![
        make_event(EventPayload::AgentTaskCreated {
            task_id: task_id.clone(),
            title: "implement feature".into(),
            role: AgentRole::Worker,
            dependencies: vec![],
        }),
        make_event(EventPayload::AgentTaskCompleted {
            task_id: task_id.clone(),
        }),
    ];

    let session_id = SessionId::new();
    let checkpoint = CheckpointWriter::build_checkpoint(
        &events,
        &sample_goal(),
        &session_id,
        0,
        Some("abc123".into()),
        vec![],
    );

    assert_eq!(checkpoint.session_index, 0);
    assert_eq!(checkpoint.git_sha.as_deref(), Some("abc123"));
    assert!(checkpoint
        .completed_items
        .contains(&"implement feature".to_string()));
    assert!(checkpoint.checkpoint_id.starts_with("ckpt_"));
}

#[test]
fn remaining_items_exclude_completed() {
    let task_id = TaskId::new();
    let events = vec![
        make_event(EventPayload::AgentTaskCreated {
            task_id: task_id.clone(),
            title: "tests pass".into(),
            role: AgentRole::Worker,
            dependencies: vec![],
        }),
        make_event(EventPayload::AgentTaskCompleted {
            task_id: task_id.clone(),
        }),
    ];

    let checkpoint = CheckpointWriter::build_checkpoint(
        &events,
        &sample_goal(),
        &SessionId::new(),
        0,
        None,
        vec![],
    );

    assert!(!checkpoint
        .remaining_items
        .contains(&"tests pass".to_string()));
    assert!(checkpoint
        .remaining_items
        .contains(&"no lint errors".to_string()));
    assert!(checkpoint
        .remaining_items
        .contains(&"docs updated".to_string()));
}

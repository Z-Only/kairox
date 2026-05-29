use super::*;
use crate::{AgentId, AgentRole, EventPayload, PrivacyClassification, SessionId, WorkspaceId};

#[test]
fn projects_user_and_assistant_messages() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let events = vec![
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::UserMessageAdded {
                message_id: "m1".into(),
                content: "hello".into(),
            },
        ),
        DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::AssistantMessageCompleted {
                message_id: "m2".into(),
                content: "hi".into(),
            },
        ),
    ];

    let projection = SessionProjection::from_events(&events);

    assert_eq!(projection.messages.len(), 2);
    assert_eq!(projection.messages[0].role, ProjectedRole::User);
    assert_eq!(projection.messages[1].content, "hi");
}

#[test]
fn serializes_projection_with_snake_case_roles() {
    let projection = SessionProjection {
        messages: vec![ProjectedMessage {
            role: ProjectedRole::Assistant,
            content: "hello".into(),
        }],
        task_titles: vec!["inspect repo".into()],
        task_graph: TaskGraphSnapshot::default(),
        token_stream: "hello".into(),
        cancelled: true,
        last_context_usage: None,
        model_limits: None,
        compaction: CompactionStatus::default(),
    };

    let json = serde_json::to_value(&projection).unwrap();

    assert_eq!(json["messages"][0]["role"], "assistant");
    assert_eq!(json["messages"][0]["content"], "hello");
    assert_eq!(json["task_titles"][0], "inspect repo");
    assert_eq!(json["token_stream"], "hello");
    assert_eq!(json["cancelled"], true);

    let round_tripped: SessionProjection = serde_json::from_value(json).unwrap();
    assert_eq!(round_tripped, projection);
}

#[test]
fn projects_token_deltas_tasks_and_cancellation() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let task_id = crate::TaskId::new();
    let events = vec![
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::ModelTokenDelta {
                delta: "hel".into(),
            },
        ),
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::ModelTokenDelta { delta: "lo".into() },
        ),
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::planner(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentTaskCreated {
                task_id,
                title: "inspect repo".into(),
                role: AgentRole::Planner,
                dependencies: vec![],
            },
        ),
        DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::SessionCancelled {
                reason: "user stopped".into(),
            },
        ),
    ];

    let projection = SessionProjection::from_events(&events);

    assert_eq!(projection.token_stream, "hello");
    assert_eq!(projection.task_titles, vec!["inspect repo"]);
    assert!(projection.cancelled);
}

#[test]
fn compaction_status_serializes_with_internal_tag() {
    let s = CompactionStatus::Idle;
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "Idle");

    let s = CompactionStatus::Running;
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "Running");

    let s = CompactionStatus::Failed {
        error: "llm timeout".into(),
    };
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "Failed");
    assert_eq!(json["error"], "llm timeout");

    let back: CompactionStatus = serde_json::from_value(json).unwrap();
    assert!(matches!(back, CompactionStatus::Failed { .. }));
}

#[test]
fn compaction_status_default_is_idle() {
    let s = CompactionStatus::default();
    assert!(matches!(s, CompactionStatus::Idle));
}

#[test]
fn projects_context_assembled_into_last_context_usage() {
    use crate::context_types::{ContextSource, ContextUsage};
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let usage = ContextUsage {
        total_tokens: 12_000,
        budget_tokens: 180_000,
        context_window: 200_000,
        output_reservation: 20_000,
        by_source: vec![
            (ContextSource::System, 2_000),
            (ContextSource::History, 10_000),
        ],
        estimator: "cl100k_base".to_string(),
        corrected_by_real_usage: false,
    };

    let event = DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextAssembled {
            usage: usage.clone(),
        },
    );

    let projection = SessionProjection::from_events(&[event]);

    let cached = projection.last_context_usage.expect("usage should be set");
    assert_eq!(cached.total_tokens, 12_000);
    assert_eq!(cached.budget_tokens, 180_000);
    assert_eq!(cached.by_source.len(), 2);
}

#[test]
fn projects_compaction_lifecycle_into_compaction_status() {
    use crate::events::CompactionReason;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let started = DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextCompactionStarted {
            reason: CompactionReason::UserRequested,
            before_tokens: 180_000u64,
            candidate_event_count: 42usize,
        },
    );
    let completed = DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextCompactionCompleted {
            summary_id: "sum_1".into(),
            after_tokens: 30_000u64,
            fallback_used: false,
        },
    );

    let only_started = SessionProjection::from_events(std::slice::from_ref(&started));
    assert!(matches!(only_started.compaction, CompactionStatus::Running));

    let started_then_done = SessionProjection::from_events(&[started, completed]);
    assert!(matches!(
        started_then_done.compaction,
        CompactionStatus::Idle
    ));
}

#[test]
fn projects_compaction_failed_into_failed_status() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let failed = DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextCompactionFailed {
            error: "model timeout".into(),
            fallback_used: true,
        },
    );

    let projection = SessionProjection::from_events(&[failed]);
    match projection.compaction {
        CompactionStatus::Failed { error } => assert_eq!(error, "model timeout"),
        other => panic!("expected Failed, got {other:?}"),
    }
}

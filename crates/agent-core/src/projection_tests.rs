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
                display_content: None,
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
fn projects_user_message_display_content_when_present() {
    let events = vec![DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::UserMessageAdded {
            message_id: "m1".into(),
            content: "```md\n// file: notes.md\nsecret\n```".into(),
            display_content: Some("@notes.md summarize this".into()),
        },
    )];

    let projection = SessionProjection::from_events(&events);

    assert_eq!(projection.messages.len(), 1);
    assert_eq!(projection.messages[0].role, ProjectedRole::User);
    assert_eq!(projection.messages[0].content, "@notes.md summarize this");
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
fn clears_cancelled_state_when_a_followup_turn_starts() {
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
                content: "long request".into(),
                display_content: None,
            },
        ),
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::SessionCancelled {
                reason: "user stopped".into(),
            },
        ),
        DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::UserMessageAdded {
                message_id: "m2".into(),
                content: "continue".into(),
                display_content: None,
            },
        ),
    ];

    let projection = SessionProjection::from_events(&events);

    assert!(!projection.cancelled);
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

// --- Helper to create a task and return the event + task_id ---
fn make_task_created_event(
    workspace_id: &WorkspaceId,
    session_id: &SessionId,
    task_id: &crate::TaskId,
    title: &str,
) -> DomainEvent {
    DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::planner(),
        PrivacyClassification::MinimalTrace,
        EventPayload::AgentTaskCreated {
            task_id: task_id.clone(),
            title: title.into(),
            role: AgentRole::Worker,
            dependencies: vec![],
        },
    )
}

#[test]
fn projects_agent_task_started() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let task_id = crate::TaskId::new();

    let events = vec![
        make_task_created_event(&workspace_id, &session_id, &task_id, "build feature"),
        DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::AgentTaskStarted {
                task_id: task_id.clone(),
            },
        ),
    ];

    let projection = SessionProjection::from_events(&events);
    let task = projection
        .task_graph
        .tasks
        .iter()
        .find(|t| t.id == task_id)
        .expect("task should exist");
    assert_eq!(task.state, TaskState::Running);
}

#[test]
fn projects_agent_task_completed() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let task_id = crate::TaskId::new();

    let events = vec![
        make_task_created_event(&workspace_id, &session_id, &task_id, "run tests"),
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::AgentTaskStarted {
                task_id: task_id.clone(),
            },
        ),
        DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::AgentTaskCompleted {
                task_id: task_id.clone(),
            },
        ),
    ];

    let projection = SessionProjection::from_events(&events);
    let task = projection
        .task_graph
        .tasks
        .iter()
        .find(|t| t.id == task_id)
        .expect("task should exist");
    assert_eq!(task.state, TaskState::Completed);
}

#[test]
fn projects_agent_task_failed_with_error() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let task_id = crate::TaskId::new();

    let events = vec![
        make_task_created_event(&workspace_id, &session_id, &task_id, "deploy"),
        DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::AgentTaskFailed {
                task_id: task_id.clone(),
                error: "timeout reached".into(),
            },
        ),
    ];

    let projection = SessionProjection::from_events(&events);
    let task = projection
        .task_graph
        .tasks
        .iter()
        .find(|t| t.id == task_id)
        .expect("task should exist");
    assert_eq!(task.state, TaskState::Failed);
    assert_eq!(task.error.as_deref(), Some("timeout reached"));
}

#[test]
fn projects_task_blocked() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let task_a = crate::TaskId::new();
    let task_b = crate::TaskId::new();

    let events = vec![
        make_task_created_event(&workspace_id, &session_id, &task_a, "task A"),
        make_task_created_event(&workspace_id, &session_id, &task_b, "task B"),
        DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::TaskBlocked {
                task_id: task_b.clone(),
                blocking_task_id: task_a.clone(),
                reason: "waiting on task A".into(),
            },
        ),
    ];

    let projection = SessionProjection::from_events(&events);
    let task = projection
        .task_graph
        .tasks
        .iter()
        .find(|t| t.id == task_b)
        .expect("task B should exist");
    assert_eq!(task.state, TaskState::Blocked);
    assert_eq!(task.error.as_deref(), Some("waiting on task A"));
}

#[test]
fn projects_agent_spawned_assigns_agent_id() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let task_id = crate::TaskId::new();

    let events = vec![
        make_task_created_event(&workspace_id, &session_id, &task_id, "code review"),
        DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::AgentSpawned {
                agent_id: "agent_worker_alpha".into(),
                role: "Worker".into(),
                task_id: task_id.clone(),
            },
        ),
    ];

    let projection = SessionProjection::from_events(&events);
    let task = projection
        .task_graph
        .tasks
        .iter()
        .find(|t| t.id == task_id)
        .expect("task should exist");
    assert_eq!(
        task.assigned_agent_id.as_deref(),
        Some("agent_worker_alpha")
    );
}

#[test]
fn projects_task_retried_resets_state() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let task_id = crate::TaskId::new();

    let events = vec![
        make_task_created_event(&workspace_id, &session_id, &task_id, "flaky task"),
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::AgentTaskFailed {
                task_id: task_id.clone(),
                error: "transient error".into(),
            },
        ),
        DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::TaskRetried {
                task_id: task_id.clone(),
                attempt: 2,
            },
        ),
    ];

    let projection = SessionProjection::from_events(&events);
    let task = projection
        .task_graph
        .tasks
        .iter()
        .find(|t| t.id == task_id)
        .expect("task should exist");
    assert_eq!(task.state, TaskState::Pending);
    assert_eq!(task.retry_count, 2);
    assert!(task.error.is_none(), "error should be cleared on retry");
    assert!(
        task.failure_reason.is_none(),
        "failure_reason should be cleared on retry"
    );
}

#[test]
fn projects_task_cancelled() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let task_id = crate::TaskId::new();

    let events = vec![
        make_task_created_event(&workspace_id, &session_id, &task_id, "long running"),
        DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::TaskCancelled {
                task_id: task_id.clone(),
            },
        ),
    ];

    let projection = SessionProjection::from_events(&events);
    let task = projection
        .task_graph
        .tasks
        .iter()
        .find(|t| t.id == task_id)
        .expect("task should exist");
    assert_eq!(task.state, TaskState::Cancelled);
    assert!(task.error.is_none(), "error should be None on cancel");
    assert_eq!(
        task.failure_reason,
        Some(crate::TaskFailureReason::Cancelled)
    );
}

#[test]
fn projects_model_profile_switched() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let event = DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ModelProfileSwitched {
            from_profile: "gpt-4o".into(),
            to_profile: "claude-sonnet".into(),
            reasoning_effort: Some("high".into()),
            effective_at: chrono::Utc::now(),
            context_window: 200_000,
            output_limit: 16_384,
            limit_source: "builtin_registry".into(),
        },
    );

    let projection = SessionProjection::from_events(&[event]);
    let limits = projection.model_limits.expect("model_limits should be set");
    assert_eq!(limits.context_window, 200_000);
    assert_eq!(limits.output_limit, 16_384);
    assert_eq!(limits.source, "builtin_registry");
}

#[test]
fn session_initialized_is_noop() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let event = DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::SessionInitialized {
            model_profile: "claude-sonnet".into(),
        },
    );

    let projection = SessionProjection::from_events(&[event]);
    assert!(projection.messages.is_empty());
    assert!(projection.task_titles.is_empty());
    assert!(projection.task_graph.tasks.is_empty());
    assert!(!projection.cancelled);
    assert!(projection.last_context_usage.is_none());
    assert!(projection.model_limits.is_none());
}

#[test]
fn task_decomposed_is_noop() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let parent_task = crate::TaskId::new();
    let sub_task = crate::TaskId::new();

    let event = DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::planner(),
        PrivacyClassification::MinimalTrace,
        EventPayload::TaskDecomposed {
            parent_task_id: parent_task,
            sub_task_ids: vec![sub_task],
        },
    );

    let projection = SessionProjection::from_events(&[event]);
    assert!(projection.messages.is_empty());
    assert!(projection.task_titles.is_empty());
    assert!(projection.task_graph.tasks.is_empty());
}

#[test]
fn assistant_message_completed_clears_token_stream() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let events = vec![
        DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::ModelTokenDelta {
                delta: "streaming...".into(),
            },
        ),
        DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::AssistantMessageCompleted {
                message_id: "m1".into(),
                content: "final answer".into(),
            },
        ),
    ];

    let projection = SessionProjection::from_events(&events);
    assert!(
        projection.token_stream.is_empty(),
        "token_stream should be cleared after AssistantMessageCompleted"
    );
    assert_eq!(projection.messages.len(), 1);
    assert_eq!(projection.messages[0].content, "final answer");
}

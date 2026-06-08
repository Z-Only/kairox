use agent_core::autonomous::{AutonomousConfig, AutonomousTaskGoal};
use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, TaskId, WorkspaceId,
};
use agent_store::{EventStore, SqliteAutonomousTaskStore, SqliteEventStore};
use std::sync::Arc;

use super::*;

async fn setup() -> (
    AutonomousController<SqliteEventStore>,
    Arc<SqliteEventStore>,
    Arc<SqliteAutonomousTaskStore>,
) {
    let event_store = SqliteEventStore::in_memory().await.unwrap();
    let event_store = Arc::new(event_store);
    let auto_store = SqliteAutonomousTaskStore::new(event_store.pool().clone());
    auto_store.migrate().await.unwrap();
    let auto_store = Arc::new(auto_store);

    let (event_tx, _) = tokio::sync::broadcast::channel(128);
    let controller = AutonomousController::new(event_store.clone(), auto_store.clone(), event_tx);
    (controller, event_store, auto_store)
}

fn sample_goal() -> AutonomousTaskGoal {
    AutonomousTaskGoal {
        description: "Build feature".into(),
        acceptance_criteria: vec!["tests pass".into()],
        verification_commands: vec!["cargo test".into()],
    }
}

#[tokio::test]
async fn start_autonomous_task_creates_row_and_emits_events() {
    let (controller, store, auto_store) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let task_id = controller
        .start_autonomous_task(
            &workspace_id,
            &session_id,
            sample_goal(),
            AutonomousConfig::default(),
        )
        .await
        .unwrap();

    assert!(task_id.as_str().starts_with("atk_"));

    let row = auto_store
        .get_autonomous_task(task_id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, "active");
    assert_eq!(row.session_count, 1);
    assert_eq!(row.current_session_id.as_deref(), Some(session_id.as_str()));

    let chain = auto_store
        .list_session_chain(task_id.as_str())
        .await
        .unwrap();
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].session_id, session_id.as_str());

    let events = store.load_session(&session_id).await.unwrap();
    let event_types: Vec<_> = events.iter().map(|e| e.payload.event_type()).collect();
    assert!(event_types.contains(&"AutonomousTaskCreated"));
    assert!(event_types.contains(&"AutonomousTaskSessionStarted"));
}

#[tokio::test]
async fn cancel_autonomous_task_updates_state() {
    let (controller, _, auto_store) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let task_id = controller
        .start_autonomous_task(
            &workspace_id,
            &session_id,
            sample_goal(),
            AutonomousConfig::default(),
        )
        .await
        .unwrap();

    controller
        .cancel_autonomous_task(&workspace_id, &session_id, &task_id)
        .await
        .unwrap();

    let row = auto_store
        .get_autonomous_task(task_id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, "cancelled");
}

#[tokio::test]
async fn register_continuation_session_increments_count() {
    let (controller, _, auto_store) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let task_id = controller
        .start_autonomous_task(
            &workspace_id,
            &session_id,
            sample_goal(),
            AutonomousConfig::default(),
        )
        .await
        .unwrap();

    let new_session = SessionId::new();
    controller
        .register_continuation_session(&task_id, &workspace_id, &new_session, 1)
        .await
        .unwrap();

    let row = auto_store
        .get_autonomous_task(task_id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.session_count, 2);
    assert_eq!(
        row.current_session_id.as_deref(),
        Some(new_session.as_str())
    );

    let chain = auto_store
        .list_session_chain(task_id.as_str())
        .await
        .unwrap();
    assert_eq!(chain.len(), 2);
}

// ── on_session_ended branches ───────────────────────────────────────

/// Helper: emit a domain event into the store for the given session.
async fn emit_event(
    store: &SqliteEventStore,
    workspace_id: &WorkspaceId,
    session_id: &SessionId,
    payload: EventPayload,
) {
    let event = DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        payload,
    );
    store.append(&event).await.unwrap();
}

/// Helper: start a task and seed the session with a SessionInitialized event
/// so load_session returns a non-empty vec.
async fn start_task_with_session(
    controller: &AutonomousController<SqliteEventStore>,
    store: &SqliteEventStore,
    workspace_id: &WorkspaceId,
    session_id: &SessionId,
    config: AutonomousConfig,
) -> AutonomousTaskId {
    emit_event(
        store,
        workspace_id,
        session_id,
        EventPayload::SessionInitialized {
            model_profile: "test".into(),
        },
    )
    .await;

    controller
        .start_autonomous_task(workspace_id, session_id, sample_goal(), config)
        .await
        .unwrap()
}

#[tokio::test]
async fn on_session_ended_task_completed_returns_none_and_marks_completed() {
    let (controller, store, auto_store) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let task_id = start_task_with_session(
        &controller,
        &store,
        &workspace_id,
        &session_id,
        Default::default(),
    )
    .await;

    // Seed a TaskCompleted event so detect_end_reason returns TaskCompleted
    let root_task = TaskId::new();
    emit_event(
        &store,
        &workspace_id,
        &session_id,
        EventPayload::AgentTaskCompleted { task_id: root_task },
    )
    .await;

    let result = controller
        .on_session_ended(&workspace_id, &session_id, &task_id)
        .await
        .unwrap();

    assert!(result.is_none(), "TaskCompleted should return None");

    let row = auto_store
        .get_autonomous_task(task_id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, "completed");
}

#[tokio::test]
async fn on_session_ended_task_failed_returns_none_and_marks_failed() {
    let (controller, store, auto_store) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let task_id = start_task_with_session(
        &controller,
        &store,
        &workspace_id,
        &session_id,
        Default::default(),
    )
    .await;

    let root_task = TaskId::new();
    emit_event(
        &store,
        &workspace_id,
        &session_id,
        EventPayload::AgentTaskFailed {
            task_id: root_task,
            error: "compilation error".into(),
        },
    )
    .await;

    let result = controller
        .on_session_ended(&workspace_id, &session_id, &task_id)
        .await
        .unwrap();

    assert!(result.is_none(), "TaskFailed should return None");

    let row = auto_store
        .get_autonomous_task(task_id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, "failed");
}

#[tokio::test]
async fn on_session_ended_user_paused_returns_none_and_marks_paused() {
    let (controller, store, auto_store) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let task_id = start_task_with_session(
        &controller,
        &store,
        &workspace_id,
        &session_id,
        Default::default(),
    )
    .await;

    emit_event(
        &store,
        &workspace_id,
        &session_id,
        EventPayload::SessionCancelled {
            reason: "user paused".into(),
        },
    )
    .await;

    let result = controller
        .on_session_ended(&workspace_id, &session_id, &task_id)
        .await
        .unwrap();

    assert!(result.is_none(), "UserPaused should return None");

    let row = auto_store
        .get_autonomous_task(task_id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, "paused");
}

#[tokio::test]
async fn on_session_ended_max_iterations_without_auto_continue_pauses() {
    let (controller, store, auto_store) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let config = AutonomousConfig {
        auto_continue: false,
        max_sessions: 5,
        verification_required: false,
        git_checkpoint: false,
    };
    let task_id =
        start_task_with_session(&controller, &store, &workspace_id, &session_id, config).await;

    let root_task = TaskId::new();
    emit_event(
        &store,
        &workspace_id,
        &session_id,
        EventPayload::AgentTaskFailed {
            task_id: root_task,
            error: "max iterations exceeded".into(),
        },
    )
    .await;

    let result = controller
        .on_session_ended(&workspace_id, &session_id, &task_id)
        .await
        .unwrap();

    assert!(
        result.is_none(),
        "MaxIterations without auto_continue should return None (pause)"
    );

    let row = auto_store
        .get_autonomous_task(task_id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, "paused");
}

#[tokio::test]
async fn on_session_ended_max_iterations_with_auto_continue_returns_continuation() {
    let (controller, store, _auto_store) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let config = AutonomousConfig {
        auto_continue: true,
        max_sessions: 5,
        verification_required: false,
        git_checkpoint: false,
    };
    let task_id =
        start_task_with_session(&controller, &store, &workspace_id, &session_id, config).await;

    let root_task = TaskId::new();
    emit_event(
        &store,
        &workspace_id,
        &session_id,
        EventPayload::AgentTaskFailed {
            task_id: root_task,
            error: "max iterations exceeded".into(),
        },
    )
    .await;

    let result = controller
        .on_session_ended(&workspace_id, &session_id, &task_id)
        .await
        .unwrap();

    let action = result.expect("should return ContinuationAction");
    assert_eq!(action.session_index, 1);
    assert!(!action.orientation_prompt.is_empty());
    assert_eq!(action.goal.description, "Build feature");
}

#[tokio::test]
async fn on_session_ended_max_sessions_reached_fails() {
    let (controller, store, auto_store) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    // max_sessions = 1 means we're already at the limit after the first session
    let config = AutonomousConfig {
        auto_continue: true,
        max_sessions: 1,
        verification_required: false,
        git_checkpoint: false,
    };
    let task_id =
        start_task_with_session(&controller, &store, &workspace_id, &session_id, config).await;

    let root_task = TaskId::new();
    emit_event(
        &store,
        &workspace_id,
        &session_id,
        EventPayload::AgentTaskFailed {
            task_id: root_task,
            error: "max iterations exceeded".into(),
        },
    )
    .await;

    let result = controller
        .on_session_ended(&workspace_id, &session_id, &task_id)
        .await
        .unwrap();

    assert!(
        result.is_none(),
        "should return None when max_sessions reached"
    );

    let row = auto_store
        .get_autonomous_task(task_id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, "failed");
}

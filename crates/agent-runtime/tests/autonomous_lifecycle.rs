//! Integration tests for the autonomous controller full lifecycle:
//! start → session end (various reasons) → checkpoint → continuation or terminal state.

use agent_core::autonomous::{AutonomousConfig, AutonomousTaskGoal};
use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, TaskId, WorkspaceId,
};
use agent_runtime::autonomous::controller::AutonomousController;
use agent_store::{AutonomousTaskStore, EventStore, SqliteAutonomousTaskStore, SqliteEventStore};
use std::sync::Arc;

async fn setup() -> (
    AutonomousController<SqliteEventStore>,
    Arc<SqliteEventStore>,
    Arc<SqliteAutonomousTaskStore>,
    tokio::sync::broadcast::Sender<DomainEvent>,
) {
    let event_store = SqliteEventStore::in_memory().await.unwrap();
    let event_store = Arc::new(event_store);
    let auto_store = SqliteAutonomousTaskStore::new(event_store.pool().clone());
    auto_store.migrate().await.unwrap();
    let auto_store = Arc::new(auto_store);

    let (event_tx, _rx) = tokio::sync::broadcast::channel(256);
    let controller =
        AutonomousController::new(event_store.clone(), auto_store.clone(), event_tx.clone());
    (controller, event_store, auto_store, event_tx)
}

fn sample_goal() -> AutonomousTaskGoal {
    AutonomousTaskGoal {
        description: "Implement login page".into(),
        acceptance_criteria: vec!["tests pass".into(), "UI renders".into()],
        verification_commands: vec!["cargo test".into()],
    }
}

fn auto_continue_config(max_sessions: u32) -> AutonomousConfig {
    AutonomousConfig {
        auto_continue: true,
        max_sessions,
        verification_required: false,
        git_checkpoint: false,
    }
}

async fn emit(
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

/// Full lifecycle: start → task completed in first session → terminal state.
#[tokio::test]
async fn full_lifecycle_single_session_completion() {
    let (controller, store, auto_store, _tx) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    // Seed session
    emit(
        &store,
        &workspace_id,
        &session_id,
        EventPayload::SessionInitialized {
            model_profile: "test".into(),
        },
    )
    .await;

    let task_id = controller
        .start_autonomous_task(
            &workspace_id,
            &session_id,
            sample_goal(),
            auto_continue_config(5),
        )
        .await
        .unwrap();

    // Simulate task completion
    let root_task = TaskId::new();
    emit(
        &store,
        &workspace_id,
        &session_id,
        EventPayload::AgentTaskCreated {
            task_id: root_task.clone(),
            title: "Implement login page".into(),
            role: agent_core::AgentRole::Planner,
            dependencies: vec![],
        },
    )
    .await;
    emit(
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

    assert!(result.is_none(), "completed task should not continue");

    let row = auto_store
        .get_autonomous_task(task_id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, "completed");

    // Verify checkpoint was written
    let checkpoints = auto_store.list_checkpoints(task_id.as_str()).await.unwrap();
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].end_reason, "task_completed");

    // Verify events emitted
    let events = store.load_session(&session_id).await.unwrap();
    let event_types: Vec<_> = events.iter().map(|e| e.payload.event_type()).collect();
    assert!(event_types.contains(&"AutonomousTaskCheckpointed"));
    assert!(event_types.contains(&"AutonomousTaskCompleted"));
}

/// Multi-session lifecycle: start → context limit → continuation → completion.
#[tokio::test]
async fn multi_session_continuation_then_completion() {
    let (controller, store, auto_store, _tx) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session1 = SessionId::new();

    // Seed session 1
    emit(
        &store,
        &workspace_id,
        &session1,
        EventPayload::SessionInitialized {
            model_profile: "test".into(),
        },
    )
    .await;

    let task_id = controller
        .start_autonomous_task(
            &workspace_id,
            &session1,
            sample_goal(),
            auto_continue_config(3),
        )
        .await
        .unwrap();

    // Session 1 hits max iterations
    let root_task1 = TaskId::new();
    emit(
        &store,
        &workspace_id,
        &session1,
        EventPayload::AgentTaskFailed {
            task_id: root_task1,
            error: "max iterations exceeded".into(),
        },
    )
    .await;

    let result = controller
        .on_session_ended(&workspace_id, &session1, &task_id)
        .await
        .unwrap();

    let action = result.expect("should request continuation");
    assert_eq!(action.session_index, 1);
    assert!(!action.orientation_prompt.is_empty());

    // Register continuation session
    let session2 = SessionId::new();
    controller
        .register_continuation_session(&task_id, &workspace_id, &session2, 1)
        .await
        .unwrap();

    // Seed session 2
    emit(
        &store,
        &workspace_id,
        &session2,
        EventPayload::SessionInitialized {
            model_profile: "test".into(),
        },
    )
    .await;

    // Session 2 completes successfully
    let root_task2 = TaskId::new();
    emit(
        &store,
        &workspace_id,
        &session2,
        EventPayload::AgentTaskCompleted {
            task_id: root_task2,
        },
    )
    .await;

    let result = controller
        .on_session_ended(&workspace_id, &session2, &task_id)
        .await
        .unwrap();

    assert!(
        result.is_none(),
        "completed in session 2 should be terminal"
    );

    let row = auto_store
        .get_autonomous_task(task_id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, "completed");
    assert_eq!(row.session_count, 2);

    // Should have 2 checkpoints total
    let checkpoints = auto_store.list_checkpoints(task_id.as_str()).await.unwrap();
    assert_eq!(checkpoints.len(), 2);

    // Session chain should have 2 entries
    let chain = auto_store
        .list_session_chain(task_id.as_str())
        .await
        .unwrap();
    assert_eq!(chain.len(), 2);
}

/// Cancellation terminates the task regardless of auto_continue.
#[tokio::test]
async fn cancel_stops_autonomous_task() {
    let (controller, store, auto_store, _tx) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    emit(
        &store,
        &workspace_id,
        &session_id,
        EventPayload::SessionInitialized {
            model_profile: "test".into(),
        },
    )
    .await;

    let task_id = controller
        .start_autonomous_task(
            &workspace_id,
            &session_id,
            sample_goal(),
            auto_continue_config(5),
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

    let events = store.load_session(&session_id).await.unwrap();
    let event_types: Vec<_> = events.iter().map(|e| e.payload.event_type()).collect();
    assert!(event_types.contains(&"AutonomousTaskCancelled"));
}

/// Exhaust max_sessions: auto_continue is on but limit is reached.
#[tokio::test]
async fn max_sessions_exhausted_marks_failed() {
    let (controller, store, auto_store, _tx) = setup().await;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    emit(
        &store,
        &workspace_id,
        &session_id,
        EventPayload::SessionInitialized {
            model_profile: "test".into(),
        },
    )
    .await;

    // max_sessions = 1, so after first session there's no room for continuation
    let task_id = controller
        .start_autonomous_task(
            &workspace_id,
            &session_id,
            sample_goal(),
            auto_continue_config(1),
        )
        .await
        .unwrap();

    let root_task = TaskId::new();
    emit(
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

    assert!(result.is_none(), "should not continue at max_sessions");

    let row = auto_store
        .get_autonomous_task(task_id.as_str())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, "failed");

    let events = store.load_session(&session_id).await.unwrap();
    let event_types: Vec<_> = events.iter().map(|e| e.payload.event_type()).collect();
    assert!(event_types.contains(&"AutonomousTaskFailed"));
}

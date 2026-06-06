use agent_core::autonomous::{AutonomousConfig, AutonomousTaskGoal};
use agent_core::{SessionId, WorkspaceId};
use agent_store::{SqliteAutonomousTaskStore, SqliteEventStore};
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

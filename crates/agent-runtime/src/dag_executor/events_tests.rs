use super::*;
use agent_core::{
    AgentId, AgentRole, DomainEvent, EventPayload, PrivacyClassification, SessionId, TaskId,
    WorkspaceId,
};
use agent_store::SqliteEventStore;
use std::sync::Arc;

async fn make_emitter() -> (
    EventEmitter<SqliteEventStore>,
    Arc<SqliteEventStore>,
    tokio::sync::broadcast::Receiver<DomainEvent>,
) {
    let store = Arc::new(SqliteEventStore::in_memory().await.unwrap());
    let (tx, rx) = tokio::sync::broadcast::channel(1024);
    let emitter = EventEmitter {
        store: Arc::clone(&store),
        event_tx: tx,
    };
    (emitter, store, rx)
}

fn test_ids() -> (WorkspaceId, SessionId, TaskId) {
    (WorkspaceId::new(), SessionId::new(), TaskId::new())
}

#[tokio::test]
async fn test_emit_task_created() {
    let (emitter, store, mut rx) = make_emitter().await;
    let (workspace_id, session_id, task_id) = test_ids();
    let dep = TaskId::new();

    emitter
        .emit_task_created(
            &workspace_id,
            &session_id,
            &task_id,
            "Build feature",
            AgentRole::Worker,
            std::slice::from_ref(&dep),
        )
        .await
        .unwrap();

    // Verify broadcast
    let event = rx.try_recv().expect("event should be broadcast");
    assert_eq!(event.source_agent_id, AgentId::system());
    assert_eq!(event.privacy, PrivacyClassification::MinimalTrace);
    match &event.payload {
        EventPayload::AgentTaskCreated {
            task_id: tid,
            title,
            role,
            dependencies,
        } => {
            assert_eq!(tid, &task_id);
            assert_eq!(title, "Build feature");
            assert_eq!(role, &AgentRole::Worker);
            assert_eq!(dependencies, &[dep]);
        }
        other => panic!("expected AgentTaskCreated, got {other:?}"),
    }

    // Verify store persistence
    let events = store.load_session(&session_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0].payload,
        EventPayload::AgentTaskCreated { .. }
    ));
}

#[tokio::test]
async fn test_emit_task_started() {
    let (emitter, store, mut rx) = make_emitter().await;
    let (workspace_id, session_id, task_id) = test_ids();

    emitter
        .emit_task_started(&workspace_id, &session_id, &task_id)
        .await
        .unwrap();

    let event = rx.try_recv().expect("event should be broadcast");
    assert_eq!(event.source_agent_id, AgentId::system());
    assert_eq!(event.privacy, PrivacyClassification::MinimalTrace);
    match &event.payload {
        EventPayload::AgentTaskStarted { task_id: tid } => {
            assert_eq!(tid, &task_id);
        }
        other => panic!("expected AgentTaskStarted, got {other:?}"),
    }

    let events = store.load_session(&session_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0].payload,
        EventPayload::AgentTaskStarted { .. }
    ));
}

#[tokio::test]
async fn test_emit_task_completed() {
    let (emitter, store, mut rx) = make_emitter().await;
    let (workspace_id, session_id, task_id) = test_ids();

    emitter
        .emit_task_completed(&workspace_id, &session_id, &task_id)
        .await
        .unwrap();

    let event = rx.try_recv().expect("event should be broadcast");
    assert_eq!(event.source_agent_id, AgentId::system());
    assert_eq!(event.privacy, PrivacyClassification::MinimalTrace);
    match &event.payload {
        EventPayload::AgentTaskCompleted { task_id: tid } => {
            assert_eq!(tid, &task_id);
        }
        other => panic!("expected AgentTaskCompleted, got {other:?}"),
    }

    let events = store.load_session(&session_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0].payload,
        EventPayload::AgentTaskCompleted { .. }
    ));
}

#[tokio::test]
async fn test_emit_task_failed() {
    let (emitter, store, mut rx) = make_emitter().await;
    let (workspace_id, session_id, task_id) = test_ids();

    emitter
        .emit_task_failed(&workspace_id, &session_id, &task_id, "timeout exceeded")
        .await
        .unwrap();

    let event = rx.try_recv().expect("event should be broadcast");
    assert_eq!(event.source_agent_id, AgentId::system());
    assert_eq!(event.privacy, PrivacyClassification::MinimalTrace);
    match &event.payload {
        EventPayload::AgentTaskFailed {
            task_id: tid,
            error,
        } => {
            assert_eq!(tid, &task_id);
            assert_eq!(error, "timeout exceeded");
        }
        other => panic!("expected AgentTaskFailed, got {other:?}"),
    }

    let events = store.load_session(&session_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0].payload,
        EventPayload::AgentTaskFailed { .. }
    ));
}

#[tokio::test]
async fn test_emit_task_cancelled() {
    let (emitter, store, mut rx) = make_emitter().await;
    let (workspace_id, session_id, task_id) = test_ids();

    emitter
        .emit_task_cancelled(&workspace_id, &session_id, &task_id)
        .await
        .unwrap();

    let event = rx.try_recv().expect("event should be broadcast");
    assert_eq!(event.source_agent_id, AgentId::system());
    assert_eq!(event.privacy, PrivacyClassification::MinimalTrace);
    match &event.payload {
        EventPayload::TaskCancelled { task_id: tid } => {
            assert_eq!(tid, &task_id);
        }
        other => panic!("expected TaskCancelled, got {other:?}"),
    }

    let events = store.load_session(&session_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0].payload,
        EventPayload::TaskCancelled { .. }
    ));
}

#[tokio::test]
async fn test_emit_task_blocked() {
    let (emitter, store, mut rx) = make_emitter().await;
    let (workspace_id, session_id, task_id) = test_ids();
    let blocking_id = TaskId::new();

    emitter
        .emit_task_blocked(
            &workspace_id,
            &session_id,
            &task_id,
            &blocking_id,
            "dependency failed",
        )
        .await
        .unwrap();

    let event = rx.try_recv().expect("event should be broadcast");
    assert_eq!(event.source_agent_id, AgentId::system());
    assert_eq!(event.privacy, PrivacyClassification::MinimalTrace);
    match &event.payload {
        EventPayload::TaskBlocked {
            task_id: tid,
            blocking_task_id,
            reason,
        } => {
            assert_eq!(tid, &task_id);
            assert_eq!(blocking_task_id, &blocking_id);
            assert_eq!(reason, "dependency failed");
        }
        other => panic!("expected TaskBlocked, got {other:?}"),
    }

    let events = store.load_session(&session_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0].payload,
        EventPayload::TaskBlocked { .. }
    ));
}

#[tokio::test]
async fn test_emit_agent_spawned() {
    let (emitter, store, mut rx) = make_emitter().await;
    let (workspace_id, session_id, task_id) = test_ids();
    let agent_id = AgentId::worker("worker-42");

    emitter
        .emit_agent_spawned(
            &workspace_id,
            &session_id,
            &agent_id,
            AgentRole::Worker,
            &task_id,
        )
        .await
        .unwrap();

    let event = rx.try_recv().expect("event should be broadcast");
    // Agent spawned uses the passed-in agent_id, NOT system()
    assert_eq!(event.source_agent_id, agent_id);
    assert_eq!(event.privacy, PrivacyClassification::MinimalTrace);
    match &event.payload {
        EventPayload::AgentSpawned {
            agent_id: aid,
            role,
            task_id: tid,
        } => {
            assert_eq!(aid, &agent_id.to_string());
            assert_eq!(role, &AgentRole::Worker.to_string());
            assert_eq!(tid, &task_id);
        }
        other => panic!("expected AgentSpawned, got {other:?}"),
    }

    let events = store.load_session(&session_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0].payload,
        EventPayload::AgentSpawned { .. }
    ));
}

#[tokio::test]
async fn test_emit_agent_idle() {
    let (emitter, store, mut rx) = make_emitter().await;
    let (workspace_id, session_id, _) = test_ids();
    let agent_id = AgentId::worker("worker-99");

    emitter
        .emit_agent_idle(&workspace_id, &session_id, &agent_id)
        .await
        .unwrap();

    let event = rx.try_recv().expect("event should be broadcast");
    // Agent idle uses the passed-in agent_id, NOT system()
    assert_eq!(event.source_agent_id, agent_id);
    assert_eq!(event.privacy, PrivacyClassification::MinimalTrace);
    match &event.payload {
        EventPayload::AgentIdle { agent_id: aid } => {
            assert_eq!(aid, &agent_id.to_string());
        }
        other => panic!("expected AgentIdle, got {other:?}"),
    }

    let events = store.load_session(&session_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0].payload, EventPayload::AgentIdle { .. }));
}

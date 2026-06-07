use super::*;
use crate::dag_executor::config::DagConfig;
use crate::dag_executor::events::EventEmitter;
use agent_core::{AgentRole, FailurePolicy, TaskState, WorkspaceId};
use agent_store::SqliteEventStore;
use std::sync::Arc;

async fn test_emitter() -> EventEmitter<SqliteEventStore> {
    let store = Arc::new(SqliteEventStore::in_memory().await.unwrap());
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    EventEmitter {
        store,
        event_tx: tx,
    }
}

fn test_ids() -> (WorkspaceId, agent_core::SessionId) {
    (
        WorkspaceId::from_string("wrk_recovery_unit".to_string()),
        agent_core::SessionId::new(),
    )
}

#[tokio::test]
async fn apply_failure_policy_block_dependents() {
    let emitter = test_emitter().await;
    let (workspace_id, session_id) = test_ids();
    let config = DagConfig {
        failure_policy: FailurePolicy::BlockDependents,
        ..Default::default()
    };

    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Worker, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);

    graph.mark_running(&a).unwrap();
    graph.mark_failed(&a, "boom".into()).unwrap();

    apply_failure_policy(
        &emitter,
        &config,
        &workspace_id,
        &session_id,
        &mut graph,
        &a,
    )
    .await
    .unwrap();

    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Blocked);
}

#[tokio::test]
async fn apply_failure_policy_allow_orphans() {
    let emitter = test_emitter().await;
    let (workspace_id, session_id) = test_ids();
    let config = DagConfig {
        failure_policy: FailurePolicy::AllowOrphans,
        ..Default::default()
    };

    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Worker, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);

    graph.mark_running(&a).unwrap();
    graph.mark_failed(&a, "boom".into()).unwrap();

    apply_failure_policy(
        &emitter,
        &config,
        &workspace_id,
        &session_id,
        &mut graph,
        &a,
    )
    .await
    .unwrap();

    // AllowOrphans: dependent B should NOT be blocked
    assert_ne!(graph.get_task(&b).unwrap().state, TaskState::Blocked);
}

#[tokio::test]
async fn apply_failure_policy_fail_fast() {
    let emitter = test_emitter().await;
    let (workspace_id, session_id) = test_ids();
    let config = DagConfig {
        failure_policy: FailurePolicy::FailFast,
        ..Default::default()
    };

    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Worker, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Worker, vec![]);

    graph.mark_running(&a).unwrap();
    graph.mark_failed(&a, "fatal".into()).unwrap();

    apply_failure_policy(
        &emitter,
        &config,
        &workspace_id,
        &session_id,
        &mut graph,
        &a,
    )
    .await
    .unwrap();

    // FailFast: all non-terminal tasks (B, C) should be cancelled
    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Cancelled);
    assert_eq!(graph.get_task(&c).unwrap().state, TaskState::Cancelled);
}

#[tokio::test]
async fn retry_task_resets_to_pending() {
    let emitter = test_emitter().await;
    let (workspace_id, session_id) = test_ids();

    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Worker, vec![]);
    graph.mark_running(&a).unwrap();
    graph.mark_failed(&a, "transient error".into()).unwrap();

    retry_task(&emitter, &workspace_id, &session_id, &mut graph, &a)
        .await
        .unwrap();

    let task = graph.get_task(&a).unwrap();
    assert_eq!(task.state, TaskState::Pending);
    assert_eq!(task.retry_count, 1);
    assert!(task.error.is_none());
}

#[tokio::test]
async fn retry_task_fails_on_non_retriable_state() {
    let emitter = test_emitter().await;
    let (workspace_id, session_id) = test_ids();

    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Worker, vec![]);
    graph.mark_running(&a).unwrap();
    graph.mark_completed(&a).unwrap();

    let result = retry_task(&emitter, &workspace_id, &session_id, &mut graph, &a).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn cancel_task_marks_cancelled_and_cascades() {
    let emitter = test_emitter().await;
    let (workspace_id, session_id) = test_ids();
    let config = DagConfig::default(); // BlockDependents

    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Worker, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);

    cancel_task(
        &emitter,
        &config,
        &workspace_id,
        &session_id,
        &mut graph,
        &a,
    )
    .await
    .unwrap();

    assert_eq!(graph.get_task(&a).unwrap().state, TaskState::Cancelled);
    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Blocked);
}

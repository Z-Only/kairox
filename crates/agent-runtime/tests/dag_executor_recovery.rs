//! Integration tests for DagExecutor retry, cancel, and failure-policy behavior.

mod support;

use agent_core::{AgentId, AgentRole, FailurePolicy, TaskState, WorkspaceId};
use agent_runtime::{DagConfig, TaskGraph};
use support::dag_executor::{make_executor, make_executor_with_config};

#[tokio::test]
async fn dag_executor_retry_task_unblocks_dependents() {
    let executor = make_executor().await;

    let mut graph = TaskGraph::default();
    let a =
        graph.add_task_with_config("A", AgentRole::Planner, vec![], 2, Some(AgentId::planner()));
    let b = graph.add_task_with_config(
        "B",
        AgentRole::Worker,
        vec![a.clone()],
        2,
        Some(AgentId::worker("w1")),
    );
    let c = graph.add_task_with_config(
        "C",
        AgentRole::Reviewer,
        vec![b.clone()],
        2,
        Some(AgentId::reviewer()),
    );

    graph.mark_running(&a).unwrap();
    graph.mark_failed(&a, "planner error".into()).unwrap();

    let dependents = graph.find_blocked_dependents(&a);
    for dep_id in &dependents {
        graph
            .mark_blocked(dep_id, format!("dependency {} failed", a))
            .unwrap();
    }
    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Blocked);
    assert_eq!(graph.get_task(&c).unwrap().state, TaskState::Blocked);

    let workspace_id = WorkspaceId::from_string("wrk_retry_test".to_string());
    let session_id = agent_core::SessionId::new();

    executor
        .retry_task(&workspace_id, &session_id, &mut graph, &a)
        .await
        .unwrap();

    let task_a = graph.get_task(&a).unwrap();
    assert_eq!(task_a.state, TaskState::Pending);
    assert_eq!(task_a.retry_count, 1);
    assert!(task_a.error.is_none());

    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Pending);
    assert_eq!(graph.get_task(&c).unwrap().state, TaskState::Pending);
}

#[tokio::test]
async fn dag_executor_retry_task_rejects_non_failed_or_blocked() {
    let executor = make_executor().await;
    let mut graph = TaskGraph::default();
    let task_id = graph.add_task("running task", AgentRole::Worker, vec![]);
    graph.mark_running(&task_id).unwrap();

    let workspace_id = WorkspaceId::from_string("wrk_retry_reject".to_string());
    let session_id = agent_core::SessionId::new();
    let result = executor
        .retry_task(&workspace_id, &session_id, &mut graph, &task_id)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn dag_executor_retry_task_rejects_after_max_retries() {
    let executor = make_executor().await;
    let mut graph = TaskGraph::default();
    let task_id = graph.add_task_with_config(
        "exhausted task",
        AgentRole::Worker,
        vec![],
        1,
        Some(AgentId::worker("w1")),
    );

    graph.mark_running(&task_id).unwrap();
    graph.mark_failed(&task_id, "first failure".into()).unwrap();

    let workspace_id = WorkspaceId::from_string("wrk_retry_exhausted".to_string());
    let session_id = agent_core::SessionId::new();

    executor
        .retry_task(&workspace_id, &session_id, &mut graph, &task_id)
        .await
        .unwrap();

    graph.mark_running(&task_id).unwrap();
    graph
        .mark_failed(&task_id, "second failure".into())
        .unwrap();

    let result = executor
        .retry_task(&workspace_id, &session_id, &mut graph, &task_id)
        .await;

    assert!(result.is_err());
    let task = graph.get_task(&task_id).unwrap();
    assert_eq!(task.state, TaskState::Failed);
    assert_eq!(task.retry_count, 1);
    assert_eq!(task.error.as_deref(), Some("second failure"));
}

#[tokio::test]
async fn dag_executor_cancel_task_cascades_block_dependents() {
    let executor = make_executor().await;

    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Reviewer, vec![b.clone()]);

    let workspace_id = WorkspaceId::from_string("wrk_cancel_cascade".to_string());
    let session_id = agent_core::SessionId::new();
    executor
        .cancel_task(&workspace_id, &session_id, &mut graph, &a)
        .await
        .unwrap();

    assert_eq!(graph.get_task(&a).unwrap().state, TaskState::Cancelled);
    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Blocked);
    assert_eq!(graph.get_task(&c).unwrap().state, TaskState::Blocked);
}

#[tokio::test]
async fn dag_executor_cancel_task_completed_is_noop() {
    let executor = make_executor().await;

    let mut graph = TaskGraph::default();
    let task_id = graph.add_task("completed task", AgentRole::Worker, vec![]);
    graph.mark_running(&task_id).unwrap();
    graph.mark_completed(&task_id).unwrap();

    let workspace_id = WorkspaceId::from_string("wrk_cancel_noop".to_string());
    let session_id = agent_core::SessionId::new();
    executor
        .cancel_task(&workspace_id, &session_id, &mut graph, &task_id)
        .await
        .unwrap();

    assert_eq!(
        graph.get_task(&task_id).unwrap().state,
        TaskState::Completed
    );
}

#[tokio::test]
async fn dag_executor_failure_policy_allow_orphans() {
    let config = DagConfig {
        failure_policy: FailurePolicy::AllowOrphans,
        ..Default::default()
    };
    let executor = make_executor_with_config(config).await;

    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    graph.mark_failed(&a, "expected failure".into()).unwrap();

    let workspace_id = WorkspaceId::from_string("wrk_allow_orphans".to_string());
    let session_id = agent_core::SessionId::new();
    executor
        .cancel_task(&workspace_id, &session_id, &mut graph, &a)
        .await
        .unwrap();

    assert_ne!(graph.get_task(&b).unwrap().state, TaskState::Blocked);
}

#[tokio::test]
async fn dag_executor_failure_policy_fail_fast() {
    let config = DagConfig {
        failure_policy: FailurePolicy::FailFast,
        ..Default::default()
    };
    let executor = make_executor_with_config(config).await;

    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Worker, vec![]);
    graph.mark_failed(&a, "fatal".into()).unwrap();

    let workspace_id = WorkspaceId::from_string("wrk_fail_fast".to_string());
    let session_id = agent_core::SessionId::new();
    executor
        .cancel_task(&workspace_id, &session_id, &mut graph, &a)
        .await
        .unwrap();

    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Cancelled);
    assert_eq!(graph.get_task(&c).unwrap().state, TaskState::Cancelled);
}

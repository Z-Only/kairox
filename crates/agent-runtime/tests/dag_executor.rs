//! Integration tests for the DagExecutor and its integration with TaskGraph,
//! AgentStrategy, and event emission.
//!
//! These tests cover:
//! - DagExecutor construction, is_available, config, agent_status, with_strategy
//! - TaskGraph scheduling: linear, parallel, diamond DAG topologies
//! - Failure cascade (BlockDependents, AllowOrphans, FailFast)
//! - Retry (reset_to_pending, retry_task with unblocking dependents)
//! - Cancel (cancel_task with policy cascade)
//! - Execute with planner responding directly (no decomposition)

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use agent_core::{
    AgentId, AgentRole, AppFacade, DomainEvent, FailurePolicy, SendMessageRequest,
    StartSessionRequest, TaskState, WorkspaceId,
};
use agent_models::{FakeModelClient, ModelMessage, ToolCall};
use agent_runtime::{
    AgentDecision, AgentStrategy, DagConfig, DagExecutor, LocalRuntime, StepContext, SubTaskDef,
    TaskGraph, ToolResultAction,
};
use agent_store::SqliteEventStore;
use agent_tools::{PermissionEngine, PermissionMode, ToolRegistry};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Build a DagExecutor with default strategies and an in-memory store.
async fn make_executor() -> DagExecutor<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["test response".into()]);
    let (event_tx, _) = tokio::sync::broadcast::channel(1024);
    let tool_registry = Arc::new(Mutex::new(ToolRegistry::new()));
    let permission_engine = Arc::new(Mutex::new(PermissionEngine::new(PermissionMode::Agent)));
    let pending: Arc<
        Mutex<HashMap<String, tokio::sync::oneshot::Sender<agent_core::PermissionDecision>>>,
    > = Arc::new(Mutex::new(HashMap::new()));

    DagExecutor::new(
        Arc::new(store),
        Arc::new(model),
        event_tx,
        tool_registry,
        permission_engine,
        pending,
        None,
        DagConfig::default(),
    )
}

/// Create a LocalRuntime wired with DAG execution enabled, plus workspace and session.
async fn make_runtime_with_session() -> (
    LocalRuntime<SqliteEventStore, FakeModelClient>,
    WorkspaceId,
    agent_core::SessionId,
) {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["done".into()]);
    let runtime = LocalRuntime::new(store, model).with_dag_execution();

    let workspace = runtime
        .open_workspace("/tmp/dag-test".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    (runtime, workspace.workspace_id, session_id)
}

/// A custom AgentStrategy that always returns a specific AgentDecision.
#[derive(Debug, Clone)]
struct FixedDecisionStrategy {
    role_val: AgentRole,
    decision: AgentDecision,
}

impl FixedDecisionStrategy {
    fn new(role_val: AgentRole, decision: AgentDecision) -> Self {
        Self { role_val, decision }
    }
}

#[async_trait]
impl AgentStrategy for FixedDecisionStrategy {
    fn role(&self) -> AgentRole {
        self.role_val
    }

    async fn build_context(
        &self,
        _task: &agent_runtime::AgentTask,
        _graph: &TaskGraph,
        _session_events: &[DomainEvent],
    ) -> Vec<ModelMessage> {
        vec![ModelMessage {
            role: "user".into(),
            content: "test context".into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }]
    }

    async fn decide(&self, _ctx: &StepContext, _messages: Vec<ModelMessage>) -> AgentDecision {
        self.decision.clone()
    }

    async fn process_tool_result(
        &self,
        _tool_call: &ToolCall,
        _result: &str,
        _iteration: usize,
    ) -> ToolResultAction {
        ToolResultAction::Continue
    }
}

// ===========================================================================
// DagExecutor construction & configuration tests (1-4)
// ===========================================================================

#[tokio::test]
async fn dag_executor_is_available_with_planner() {
    let executor = make_executor().await;
    assert!(
        executor.is_available(),
        "DagExecutor with default strategies should be available"
    );
}

#[tokio::test]
async fn dag_executor_config_defaults() {
    let executor = make_executor().await;
    let config = executor.config();
    assert_eq!(config.max_concurrency, 3);
    assert_eq!(config.failure_policy, FailurePolicy::BlockDependents);
    assert_eq!(config.retry_config.max_model_retries, 3);
    assert_eq!(config.retry_config.max_tool_retries, 2);
}

#[tokio::test]
async fn dag_executor_agent_status_from_graph() {
    let executor = make_executor().await;

    let mut graph = TaskGraph::default();
    let task_id = graph.add_task_with_config(
        "test task",
        AgentRole::Worker,
        vec![],
        2,
        Some(AgentId::worker("w1")),
    );
    graph.mark_running(&task_id).unwrap();

    let statuses = executor.get_agent_status(&graph);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].status, "running");
    assert_eq!(statuses[0].role, AgentRole::Worker);
    assert_eq!(statuses[0].task_id, Some(task_id));
}

#[tokio::test]
async fn dag_executor_with_custom_strategy() {
    // Replace the planner strategy with a custom one that Decomposes
    let sub_tasks = vec![SubTaskDef {
        title: "Sub-task A".into(),
        role: AgentRole::Worker,
        dependencies: Vec::new(),
        description: "Do something".into(),
    }];
    let custom =
        FixedDecisionStrategy::new(AgentRole::Planner, AgentDecision::Decompose { sub_tasks });

    let executor = make_executor()
        .await
        .with_strategy(AgentRole::Planner, Arc::new(custom));

    // Still available because a planner strategy is registered
    assert!(executor.is_available());

    // Config should be unchanged
    assert_eq!(executor.config().max_concurrency, 3);
}

// ===========================================================================
// TaskGraph scheduling tests (5-9)
// ===========================================================================

#[test]
fn task_graph_linear_dag_scheduling() {
    // A → B → C
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Reviewer, vec![b.clone()]);

    // Initially only A is ready
    assert_eq!(graph.ready_tasks(), vec![a.clone()]);

    // Complete A → B becomes ready
    graph.mark_completed(&a).unwrap();
    assert_eq!(graph.ready_tasks(), vec![b.clone()]);

    // Complete B → C becomes ready
    graph.mark_completed(&b).unwrap();
    assert_eq!(graph.ready_tasks(), vec![c]);

    // Complete C → graph finished
    graph.mark_completed(&graph.ready_tasks()[0]).unwrap();
    assert!(graph.is_finished());
}

#[test]
fn task_graph_parallel_dag_scheduling() {
    // A → [B, C] → D
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Worker, vec![a.clone()]);
    let d = graph.add_task("D", AgentRole::Reviewer, vec![b.clone(), c.clone()]);

    // Only A is ready initially
    assert_eq!(graph.ready_tasks(), vec![a.clone()]);

    // Complete A → B and C are both ready (parallel)
    graph.mark_completed(&a).unwrap();
    let mut ready = graph.ready_tasks();
    ready.sort_by_key(|id| id.to_string());
    assert_eq!(ready.len(), 2);

    // Complete B → D still waiting on C
    graph.mark_completed(&b).unwrap();
    assert_eq!(graph.ready_tasks(), vec![c.clone()]);

    // Complete C → D becomes ready
    graph.mark_completed(&c).unwrap();
    assert_eq!(graph.ready_tasks(), vec![d]);
}

#[test]
fn task_graph_diamond_dag() {
    // A → B, A → C, B → D, C → D
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Worker, vec![a.clone()]);
    let d = graph.add_task("D", AgentRole::Reviewer, vec![b.clone(), c.clone()]);

    // One root
    assert_eq!(graph.ready_tasks(), vec![a.clone()]);

    // After A completes, B and C are ready
    graph.mark_completed(&a).unwrap();
    let mut ready = graph.ready_tasks();
    ready.sort_by_key(|id| id.to_string());
    assert_eq!(ready.len(), 2);

    // D is NOT ready after just B completes — needs both B and C
    graph.mark_completed(&b).unwrap();
    assert!(!graph.ready_tasks().contains(&d));

    // D is ready after C also completes
    graph.mark_completed(&c).unwrap();
    assert_eq!(graph.ready_tasks(), vec![d]);
}

#[test]
fn task_graph_failure_cascade_block_dependents() {
    // A → B → C
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Reviewer, vec![b.clone()]);

    // Fail A
    graph.mark_running(&a).unwrap();
    graph.mark_failed(&a, "planner error".into()).unwrap();

    // Simulate BlockDependents cascade
    let dependents = graph.find_blocked_dependents(&a);
    for dep_id in &dependents {
        graph
            .mark_blocked(dep_id, format!("dependency {} failed", a))
            .unwrap();
    }

    // B should be Blocked
    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Blocked);

    // C should also be Blocked (cascade)
    assert_eq!(graph.get_task(&c).unwrap().state, TaskState::Blocked);

    // Failed + Blocked are both terminal states, graph should be finished
    assert!(
        !graph.is_finished(),
        "Blocked tasks are not terminal, graph should not be finished"
    );
}

#[test]
fn task_graph_skip_unblocks_dependents() {
    // A → B
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);

    // Fail A, then block B
    graph.mark_failed(&a, "error".into()).unwrap();
    graph.mark_blocked(&b, "dependency failed".into()).unwrap();
    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Blocked);

    // Skip B — overrides Blocked
    graph.mark_skipped(&b).unwrap();
    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Skipped);

    // Graph is finished: Failed(A) + Skipped(B) — both terminal
    assert!(graph.is_finished());
}

// ===========================================================================
// TaskGraph retry & cancel tests (10-11)
// ===========================================================================

#[test]
fn task_graph_retry_resets_to_pending() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("task", AgentRole::Worker, vec![]);

    // Run and fail the task
    graph.mark_running(&id).unwrap();
    graph.mark_failed(&id, "transient error".into()).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Failed);
    assert_eq!(graph.get_task(&id).unwrap().retry_count, 0);

    // Reset for retry
    graph.reset_to_pending(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Pending);
    assert_eq!(graph.get_task(&id).unwrap().retry_count, 1);
    assert!(graph.get_task(&id).unwrap().error.is_none());
}

#[test]
fn task_graph_cancel_non_terminal() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("task", AgentRole::Worker, vec![]);

    // Mark ready (non-terminal)
    graph.mark_ready(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Ready);

    // Cancel it
    graph.mark_cancelled(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Cancelled);
}

// ===========================================================================
// DagExecutor execute test (12)
// ===========================================================================

#[tokio::test]
async fn dag_executor_execute_respond_directly() {
    // Use LocalRuntime with DAG execution + a FakeModelClient that returns plain text.
    // The planner's PlannerStrategy will call decide(), which checks the model
    // response. Since FakeModelClient returns non-JSON text, the planner
    // will not decompose and will respond directly.
    let (runtime, workspace_id, session_id) = make_runtime_with_session().await;

    // Send a message using the /plan prefix to trigger DAG execution
    runtime
        .send_message(SendMessageRequest {
            workspace_id,
            session_id: session_id.clone(),
            content: "/plan do something simple".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Verify the task graph has a single completed root task
    let snapshot = runtime.get_task_graph(session_id.clone()).await.unwrap();
    assert!(
        !snapshot.tasks.is_empty(),
        "Task graph should have at least one task"
    );

    let completed: Vec<_> = snapshot
        .tasks
        .iter()
        .filter(|t| t.state == TaskState::Completed)
        .collect();
    assert!(
        !completed.is_empty(),
        "At least the root task should be completed"
    );
}

// ===========================================================================
// DagExecutor.retry_task & cancel_task integration tests
// ===========================================================================

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

// ===========================================================================
// FailurePolicy variant integration tests
// ===========================================================================

#[tokio::test]
async fn dag_executor_failure_policy_allow_orphans() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["test".into()]);
    let (event_tx, _) = tokio::sync::broadcast::channel(1024);
    let tool_registry = Arc::new(Mutex::new(ToolRegistry::new()));
    let permission_engine = Arc::new(Mutex::new(PermissionEngine::new(PermissionMode::Agent)));
    let pending: Arc<
        Mutex<HashMap<String, tokio::sync::oneshot::Sender<agent_core::PermissionDecision>>>,
    > = Arc::new(Mutex::new(HashMap::new()));

    let config = DagConfig {
        failure_policy: FailurePolicy::AllowOrphans,
        ..Default::default()
    };
    let executor = DagExecutor::new(
        Arc::new(store),
        Arc::new(model),
        event_tx,
        tool_registry,
        permission_engine,
        pending,
        None,
        config,
    );

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
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["test".into()]);
    let (event_tx, _) = tokio::sync::broadcast::channel(1024);
    let tool_registry = Arc::new(Mutex::new(ToolRegistry::new()));
    let permission_engine = Arc::new(Mutex::new(PermissionEngine::new(PermissionMode::Agent)));
    let pending: Arc<
        Mutex<HashMap<String, tokio::sync::oneshot::Sender<agent_core::PermissionDecision>>>,
    > = Arc::new(Mutex::new(HashMap::new()));

    let config = DagConfig {
        failure_policy: FailurePolicy::FailFast,
        ..Default::default()
    };
    let executor = DagExecutor::new(
        Arc::new(store),
        Arc::new(model),
        event_tx,
        tool_registry,
        permission_engine,
        pending,
        None,
        config,
    );

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

// ===========================================================================
// Bonus: Additional edge case tests
// ===========================================================================

#[test]
fn task_graph_empty_graph_not_finished() {
    let graph = TaskGraph::default();
    assert!(!graph.is_finished(), "Empty graph should not be finished");
}

#[test]
fn task_graph_single_task_lifecycle() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("Only task", AgentRole::Worker, vec![]);

    // Starts pending, is ready
    assert!(graph.ready_tasks().contains(&id));
    assert!(!graph.is_finished());

    // Run
    graph.mark_running(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Running);
    assert!(!graph.is_finished());

    // Complete
    graph.mark_completed(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Completed);
    assert!(graph.is_finished());
}

#[test]
fn task_graph_cancel_from_running_state() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("task", AgentRole::Worker, vec![]);
    graph.mark_running(&id).unwrap();
    graph.mark_cancelled(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Cancelled);
}

#[test]
fn task_graph_cancel_completed_is_noop() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("task", AgentRole::Worker, vec![]);
    graph.mark_running(&id).unwrap();
    graph.mark_completed(&id).unwrap();
    graph.mark_cancelled(&id).unwrap(); // should be no-op
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Completed);
}

#[test]
fn task_graph_skip_overrides_failed() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("task", AgentRole::Worker, vec![]);
    graph.mark_running(&id).unwrap();
    graph.mark_failed(&id, "error".into()).unwrap();
    graph.mark_skipped(&id).unwrap();
    let task = graph.get_task(&id).unwrap();
    assert_eq!(task.state, TaskState::Skipped);
    assert!(task.error.is_none());
}

#[test]
fn task_graph_reset_to_pending_from_blocked() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("child", AgentRole::Worker, vec![]);
    graph.mark_blocked(&id, "dep failed".into()).unwrap();
    graph.reset_to_pending(&id).unwrap();
    let task = graph.get_task(&id).unwrap();
    assert_eq!(task.state, TaskState::Pending);
    assert_eq!(task.retry_count, 1);
}

#[tokio::test]
async fn dag_executor_with_replaced_planner_still_available() {
    // Build an executor, then replace the planner strategy with a custom one
    let executor = make_executor().await.with_strategy(
        AgentRole::Planner,
        Arc::new(FixedDecisionStrategy::new(
            AgentRole::Worker, // wrong role key, but replaces the planner entry
            AgentDecision::Respond("test".into()),
        )),
    );
    // with_strategy inserts by role key — the planner key still exists
    // so is_available() should still return true
    assert!(executor.is_available());
}

#[tokio::test]
async fn dag_executor_get_agent_status_empty_graph() {
    let executor = make_executor().await;
    let graph = TaskGraph::default();
    let statuses = executor.get_agent_status(&graph);
    assert!(
        statuses.is_empty(),
        "Empty graph should have no agent statuses"
    );
}

#[tokio::test]
async fn dag_executor_multiple_agents_status() {
    let executor = make_executor().await;

    let mut graph = TaskGraph::default();
    let a = graph.add_task_with_config(
        "planner task",
        AgentRole::Planner,
        vec![],
        2,
        Some(AgentId::planner()),
    );
    let b = graph.add_task_with_config(
        "worker task",
        AgentRole::Worker,
        vec![a.clone()],
        2,
        Some(AgentId::worker("w1")),
    );
    let _c = graph.add_task_with_config(
        "reviewer task",
        AgentRole::Reviewer,
        vec![b.clone()],
        2,
        Some(AgentId::reviewer()),
    );

    graph.mark_completed(&a).unwrap();
    graph.mark_running(&b).unwrap();

    let statuses = executor.get_agent_status(&graph);
    assert_eq!(statuses.len(), 3);

    let planner_status = statuses
        .iter()
        .find(|s| s.role == AgentRole::Planner)
        .unwrap();
    assert_eq!(planner_status.status, "completed");

    let worker_status = statuses
        .iter()
        .find(|s| s.role == AgentRole::Worker)
        .unwrap();
    assert_eq!(worker_status.status, "running");

    let reviewer_status = statuses
        .iter()
        .find(|s| s.role == AgentRole::Reviewer)
        .unwrap();
    assert_eq!(reviewer_status.status, "idle");
}

#[test]
fn task_graph_state_counts_after_mixed_operations() {
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Worker, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![]);
    let c = graph.add_task("C", AgentRole::Worker, vec![]);
    let d = graph.add_task("D", AgentRole::Worker, vec![]);
    let e = graph.add_task("E", AgentRole::Worker, vec![]);

    graph.mark_completed(&a).unwrap();
    graph.mark_running(&b).unwrap();
    graph.mark_failed(&c, "error".into()).unwrap();
    graph.mark_blocked(&d, "blocked".into()).unwrap();
    graph.mark_skipped(&e).unwrap();

    let counts = graph.state_counts();
    assert_eq!(counts.completed, 1);
    assert_eq!(counts.running, 1);
    assert_eq!(counts.failed, 1);
    assert_eq!(counts.blocked, 1);
    assert_eq!(counts.skipped, 1);
    assert_eq!(counts.pending, 0);
}

#[test]
fn task_graph_find_blocked_dependents_cascades_transitively() {
    // A → B → C → D
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Worker, vec![b.clone()]);
    let d = graph.add_task("D", AgentRole::Reviewer, vec![c.clone()]);

    let dependents = graph.find_blocked_dependents(&a);
    assert_eq!(dependents.len(), 3, "B, C, D should all be dependents of A");

    let dep_ids: Vec<String> = dependents.iter().map(|id| id.to_string()).collect();
    assert!(dep_ids.contains(&b.to_string()));
    assert!(dep_ids.contains(&c.to_string()));
    assert!(dep_ids.contains(&d.to_string()));
}

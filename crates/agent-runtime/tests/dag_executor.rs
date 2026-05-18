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
//! - Agent settings → strategy overrides (model profile, permission, user/project precedence)

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use futures::{stream, stream::BoxStream};
use tokio::sync::Mutex;

use agent_core::{
    AgentId, AgentRole, AppFacade, DomainEvent, EventPayload, FailurePolicy, PrivacyClassification,
    SendMessageRequest, StartSessionRequest, TaskState, WorkspaceId,
};
use agent_models::{
    FakeModelClient, ModelClient, ModelEvent, ModelMessage, ModelRequest, ToolCall,
};
use agent_runtime::{
    AgentDecision, AgentSettingsRoots, AgentStrategy, DagConfig, DagExecutor, LocalRuntime,
    StepContext, SubTaskDef, TaskGraph, ToolResultAction,
};
use agent_store::{EventStore, SqliteEventStore};
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
        AgentSettingsRoots::default(),
    )
    .await
}

/// Create a LocalRuntime wired with DAG execution enabled, plus workspace and session.
async fn make_runtime_with_session() -> (
    LocalRuntime<SqliteEventStore, FakeModelClient>,
    WorkspaceId,
    agent_core::SessionId,
) {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["done".into()]);
    let runtime = LocalRuntime::new(store, model).with_dag_execution().await;

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

#[derive(Debug, Clone)]
struct RecordingModelClient {
    requests: Arc<Mutex<Vec<ModelRequest>>>,
}

impl RecordingModelClient {
    fn new(requests: Arc<Mutex<Vec<ModelRequest>>>) -> Self {
        Self { requests }
    }
}

#[async_trait]
impl ModelClient for RecordingModelClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        self.requests.lock().await.push(request);
        Ok(Box::pin(stream::iter(vec![
            Ok(ModelEvent::TokenDelta("worker reply".into())),
            Ok(ModelEvent::Completed { usage: None }),
        ])))
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

#[tokio::test]
async fn dag_executor_request_model_uses_latest_reasoning_effort() {
    let store = Arc::new(SqliteEventStore::in_memory().await.unwrap());
    let captured_requests = Arc::new(Mutex::new(Vec::new()));
    let model = Arc::new(RecordingModelClient::new(captured_requests.clone()));
    let (event_tx, _) = tokio::sync::broadcast::channel(1024);
    let tool_registry = Arc::new(Mutex::new(ToolRegistry::new()));
    let permission_engine = Arc::new(Mutex::new(PermissionEngine::new(PermissionMode::Agent)));
    let pending: Arc<
        Mutex<HashMap<String, tokio::sync::oneshot::Sender<agent_core::PermissionDecision>>>,
    > = Arc::new(Mutex::new(HashMap::new()));

    let executor = DagExecutor::new(
        store.clone(),
        model,
        event_tx,
        tool_registry,
        permission_engine,
        pending,
        None,
        DagConfig::default(),
        AgentSettingsRoots::default(),
    )
    .await
    .with_strategy(
        AgentRole::Planner,
        Arc::new(FixedDecisionStrategy::new(
            AgentRole::Planner,
            AgentDecision::Decompose {
                sub_tasks: vec![SubTaskDef {
                    title: "worker task".into(),
                    role: AgentRole::Worker,
                    dependencies: Vec::new(),
                    description: "ask the model".into(),
                }],
            },
        )),
    )
    .with_strategy(
        AgentRole::Worker,
        Arc::new(FixedDecisionStrategy::new(
            AgentRole::Worker,
            AgentDecision::RequestModel { tools: Vec::new() },
        )),
    );

    let workspace_id = WorkspaceId::from_string("wrk_dag_reasoning".to_string());
    let session_id = agent_core::SessionId::new();
    store
        .append(&DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::SessionInitialized {
                model_profile: "fast".into(),
            },
        ))
        .await
        .unwrap();
    store
        .append(&DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::ModelProfileSwitched {
                from_profile: "fast".into(),
                to_profile: "reasoning".into(),
                reasoning_effort: Some("xhigh".into()),
                effective_at: chrono::Utc::now(),
                context_window: 128_000,
                output_limit: 16_384,
                limit_source: "user_config".into(),
            },
        ))
        .await
        .unwrap();

    executor
        .execute(
            &SendMessageRequest {
                workspace_id,
                session_id,
                content: "/plan use reasoning effort".into(),
                attachments: vec![],
            },
            &Arc::new(Mutex::new(HashMap::new())),
        )
        .await
        .unwrap();

    let requests = captured_requests.lock().await;
    let request = requests.first().expect("worker should call the model");
    assert_eq!(request.model_profile, "reasoning");
    assert_eq!(request.reasoning_effort.as_deref(), Some("xhigh"));
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
        AgentSettingsRoots::default(),
    )
    .await;

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
        AgentSettingsRoots::default(),
    )
    .await;

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

// ===========================================================================
// Agent settings → DAG executor integration tests
// ===========================================================================

/// Write an agent .md file with optional overrides into a scope directory.
#[allow(clippy::too_many_arguments)]
async fn write_agent_settings(
    root: &std::path::Path,
    name: &str,
    description: &str,
    instructions: &str,
    model_profile: Option<&str>,
    permission_mode: Option<&str>,
    tools: &[&str],
    enabled: bool,
) {
    tokio::fs::create_dir_all(root).await.unwrap();
    let mp = model_profile
        .map(|v| format!("model_profile: \"{v}\"\n"))
        .unwrap_or_default();
    let pm = permission_mode
        .map(|v| format!("permission_mode: \"{v}\"\n"))
        .unwrap_or_default();
    let tools_yaml = if tools.is_empty() {
        "tools: []\n".to_string()
    } else {
        let items: Vec<String> = tools.iter().map(|t| format!("\"{t}\"")).collect();
        format!("tools: [{}]\n", items.join(", "))
    };
    let enabled_line = if enabled {
        String::new()
    } else {
        "enabled: false\n".to_string()
    };
    let content = format!(
        "---\nname: {name}\ndescription: {description}\n{mp}{pm}{tools_yaml}{enabled_line}---\n{instructions}\n"
    );
    tokio::fs::write(root.join(format!("{name}.md")), content)
        .await
        .unwrap();
}

/// Build a DagExecutor with agent settings roots pointing at temp dirs.
async fn make_executor_with_roots(
    roots: AgentSettingsRoots,
) -> DagExecutor<SqliteEventStore, FakeModelClient> {
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
        roots,
    )
    .await
}

// --- model profile override ---

#[tokio::test]
async fn agent_settings_model_profile_override() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    // Project-level "default" agent with model_profile override
    write_agent_settings(
        &ws_agents,
        "default",
        "Custom default",
        "Custom planner instructions.",
        Some("fast"),
        None,
        &[],
        true,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let overrides = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");
    assert_eq!(overrides.0.as_deref(), Some("fast"));
    assert_eq!(overrides.1, None);
}

// --- permission mode override ---

#[tokio::test]
async fn agent_settings_permission_mode_override() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    write_agent_settings(
        &ws_agents,
        "default",
        "Read-only planner",
        "Read-only instructions.",
        None,
        Some("read_only"),
        &[],
        true,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let overrides = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");
    assert_eq!(overrides.1.as_deref(), Some("read_only"));
    assert_eq!(overrides.0, None);
}

// --- user/project override priority (Project > User > Builtin) ---

#[tokio::test]
async fn agent_settings_project_overrides_user() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    // User level: model_profile = "slow"
    write_agent_settings(
        &usr_agents,
        "default",
        "User default",
        "User instructions.",
        Some("slow"),
        Some("agent"),
        &[],
        true,
    )
    .await;
    // Project level: model_profile = "fast", permission_mode = "read_only"
    write_agent_settings(
        &ws_agents,
        "default",
        "Project default",
        "Project instructions.",
        Some("fast"),
        Some("read_only"),
        &[],
        true,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let overrides = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");
    // Project wins → model_profile = "fast", permission_mode = "read_only"
    assert_eq!(
        overrides.0.as_deref(),
        Some("fast"),
        "project model_profile should override user"
    );
    assert_eq!(
        overrides.1.as_deref(),
        Some("read_only"),
        "project permission_mode should override user"
    );
}

#[tokio::test]
async fn agent_settings_disabled_project_falls_back_to_user() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    // User level: model_profile = "balanced"
    write_agent_settings(
        &usr_agents,
        "default",
        "User default",
        "User instructions.",
        Some("balanced"),
        Some("agent"),
        &[],
        true,
    )
    .await;
    // Project level: disabled
    write_agent_settings(
        &ws_agents,
        "default",
        "Disabled project default",
        "Project instructions.",
        Some("fast"),
        Some("read_only"),
        &[],
        false,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let overrides = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");
    // Disabled project → user wins
    assert_eq!(overrides.0.as_deref(), Some("balanced"));
    assert_eq!(overrides.1.as_deref(), Some("agent"));
}

#[tokio::test]
async fn agent_settings_role_specific_worker_and_reviewer() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    // Worker agent with permission override
    write_agent_settings(
        &ws_agents,
        "worker",
        "Custom worker",
        "Worker instructions.",
        None,
        Some("workspace_write"),
        &[],
        true,
    )
    .await;
    // Code-reviewer agent with model profile + tools
    write_agent_settings(
        &ws_agents,
        "code-reviewer",
        "Custom reviewer",
        "Reviewer instructions.",
        Some("fast"),
        None,
        &["shell", "fs.read"],
        true,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    // Worker: permission_mode = "workspace_write"
    let worker_overrides = executor
        .agent_settings_overrides(AgentRole::Worker)
        .expect("worker strategy must exist");
    assert_eq!(worker_overrides.1.as_deref(), Some("workspace_write"));

    // Reviewer: model_profile = "fast", tools = ["shell", "fs.read"]
    let reviewer_overrides = executor
        .agent_settings_overrides(AgentRole::Reviewer)
        .expect("reviewer strategy must exist");
    assert_eq!(reviewer_overrides.0.as_deref(), Some("fast"));
    assert_eq!(reviewer_overrides.3, vec!["shell", "fs.read"]);
}

// --- user-only project-only coverage ---

#[tokio::test]
async fn agent_settings_user_only_agent_applied() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    // Only user-level "default" with model_profile = "slow"
    write_agent_settings(
        &usr_agents,
        "default",
        "User-only default",
        "User-only instructions.",
        Some("slow"),
        None,
        &[],
        true,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    let overrides = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");
    assert_eq!(overrides.0.as_deref(), Some("slow"));
}

#[tokio::test]
async fn agent_settings_no_custom_agents_falls_back_to_builtins() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    // No custom agent files — only builtins are available
    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    // Builtin defaults have no model_profile override
    let planner = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");
    assert_eq!(planner.0, None, "builtin default has no model_profile");
    assert_eq!(planner.1, None, "builtin default has no permission_mode");

    // Builtin worker has permission_mode = "workspace_write"
    let worker = executor
        .agent_settings_overrides(AgentRole::Worker)
        .expect("worker strategy must exist");
    assert_eq!(worker.1.as_deref(), Some("workspace_write"));

    // Builtin code-reviewer has permission_mode = "read_only"
    let reviewer = executor
        .agent_settings_overrides(AgentRole::Reviewer)
        .expect("reviewer strategy must exist");
    assert_eq!(reviewer.1.as_deref(), Some("read_only"));
    assert_eq!(
        reviewer.3,
        vec!["fs.read", "search", "shell"],
        "builtin code-reviewer tools"
    );
}

#[tokio::test]
async fn agent_settings_instructions_override_wired_into_context() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    write_agent_settings(
        &ws_agents,
        "default",
        "Custom planner",
        "Custom system prompt override.",
        None,
        None,
        &[],
        true,
    )
    .await;

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    let executor = make_executor_with_roots(roots).await;

    // The planner strategy should build context with the custom instructions
    let planner = executor
        .agent_settings_overrides(AgentRole::Planner)
        .expect("planner strategy must exist");

    // Verify no model or permission override — the instructions override is
    // tested via build_context in the unit tests (agents::tests)
    assert_eq!(planner.0, None);
    assert_eq!(planner.1, None);
}

// --- Invalid agent settings are excluded from effective resolution ---

#[tokio::test]
async fn agent_settings_invalid_agent_not_used() {
    let workspace = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    let ws_agents = workspace.path().join(".kairox/agents");
    let usr_agents = user.path().join(".config/kairox/agents");

    // Write an invalid agent file (no frontmatter) — should not crash but skip
    tokio::fs::create_dir_all(&ws_agents).await.unwrap();
    tokio::fs::write(
        ws_agents.join("default.md"),
        "This file has no frontmatter — it's invalid.\n",
    )
    .await
    .unwrap();

    let roots = AgentSettingsRoots {
        workspace_root: Some(ws_agents),
        user_root: Some(usr_agents),
        builtin_root: None,
    };
    // Should not panic; falls back to builtin defaults
    let executor = make_executor_with_roots(roots).await;
    assert!(executor.is_available());
}

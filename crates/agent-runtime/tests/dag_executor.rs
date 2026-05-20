//! Integration tests for DagExecutor construction, strategy wiring, execution,
//! model profile resolution, and agent status projection.

mod support;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use agent_core::{
    AgentId, AgentRole, AppFacade, FailurePolicy, SendMessageRequest, TaskState, WorkspaceId,
};
use agent_runtime::{
    AgentDecision, AgentSettingsRoots, DagConfig, DagExecutor, SubTaskDef, TaskGraph,
};
use agent_store::SqliteEventStore;
use agent_tools::{PermissionEngine, PermissionMode, ToolRegistry};
use support::dag_executor::{
    append_model_profile_events, make_executor, make_runtime_with_session, FixedDecisionStrategy,
    RecordingModelClient,
};

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

    assert!(executor.is_available());
    assert_eq!(executor.config().max_concurrency, 3);
}

#[tokio::test]
async fn dag_executor_execute_respond_directly() {
    let (runtime, workspace_id, session_id) = make_runtime_with_session().await;

    runtime
        .send_message(SendMessageRequest {
            workspace_id,
            session_id: session_id.clone(),
            content: "/plan do something simple".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

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
    append_model_profile_events(store.as_ref(), &workspace_id, &session_id).await;

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

#[tokio::test]
async fn dag_executor_with_replaced_planner_still_available() {
    let executor = make_executor().await.with_strategy(
        AgentRole::Planner,
        Arc::new(FixedDecisionStrategy::new(
            AgentRole::Worker,
            AgentDecision::Respond("test".into()),
        )),
    );
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

use super::*;
use agent_core::FailurePolicy;
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, PermissionEngine, SandboxPolicy, ToolRegistry};

async fn make_executor() -> DagExecutor<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["test".into()]);
    let (event_tx, _) = tokio::sync::broadcast::channel(1024);
    let tool_registry = Arc::new(Mutex::new(ToolRegistry::new()));
    let permission_engine = Arc::new(Mutex::new(PermissionEngine::new(
        ApprovalPolicy::OnRequest,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        },
    )));
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
        Arc::new(agent_config::Config::defaults()),
        DagConfig::default(),
        AgentSettingsRoots::default(),
    )
    .await
}

#[test]
fn dag_config_defaults() {
    let config = DagConfig::default();
    assert_eq!(config.max_concurrency, 3);
    assert_eq!(config.failure_policy, FailurePolicy::BlockDependents);
    assert_eq!(config.retry_config.max_model_retries, 3);
    assert_eq!(config.retry_config.max_tool_retries, 2);
}

#[test]
fn execution_result_from_graph() {
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Reviewer, vec![b.clone()]);

    graph.mark_completed(&a).unwrap();
    graph.mark_running(&b).unwrap();
    graph.mark_completed(&b).unwrap();
    graph.mark_running(&c).unwrap();
    graph.mark_completed(&c).unwrap();

    // Verify graph state
    let counts = graph.state_counts();
    assert_eq!(counts.completed, 3);
    assert!(graph.is_finished());
}

#[tokio::test]
async fn is_available_with_planner() {
    let executor = make_executor().await;
    assert!(executor.is_available());
}

#[tokio::test]
async fn agent_status_from_graph() {
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
}

use super::*;
use crate::agents::{AgentDecision, AgentStrategy, StepContext, ToolResultAction};
use crate::dag_executor::events::EventEmitter;
use crate::task_graph::TaskGraph;
use agent_core::{AgentId, AgentRole, DomainEvent, EventPayload, SessionId, WorkspaceId};
use agent_models::{FakeModelClient, ModelMessage, ToolCall};
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, PermissionEngine, SandboxPolicy};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// FakeStrategy — returns a preconfigured AgentDecision from `decide()`
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct FakeStrategy {
    role: AgentRole,
    decision: AgentDecision,
}

impl FakeStrategy {
    fn new(role: AgentRole, decision: AgentDecision) -> Self {
        Self { role, decision }
    }
}

#[async_trait]
impl AgentStrategy for FakeStrategy {
    fn role(&self) -> AgentRole {
        self.role
    }

    async fn build_context(
        &self,
        _task: &crate::task_graph::AgentTask,
        _graph: &TaskGraph,
        _events: &[DomainEvent],
    ) -> Vec<ModelMessage> {
        vec![]
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

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

struct TestHarness {
    events: EventEmitter<SqliteEventStore>,
    model: Arc<FakeModelClient>,
    permission_engine: Arc<Mutex<PermissionEngine>>,
    model_config: agent_config::Config,
    workspace_id: WorkspaceId,
    session_id: SessionId,
    agent_id: AgentId,
}

impl TestHarness {
    async fn new(tokens: Vec<String>) -> Self {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let (event_tx, _) = tokio::sync::broadcast::channel(1024);
        let events = EventEmitter {
            store: Arc::new(store),
            event_tx,
        };
        let model = Arc::new(FakeModelClient::new(tokens));
        let permission_engine = Arc::new(Mutex::new(PermissionEngine::new(
            ApprovalPolicy::OnRequest,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )));
        let model_config = agent_config::Config::defaults();
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();
        let agent_id = AgentId::worker("test");

        Self {
            events,
            model,
            permission_engine,
            model_config,
            workspace_id,
            session_id,
            agent_id,
        }
    }

    fn make_ctx(&self) -> StepContext {
        StepContext {
            session_id: self.session_id.clone(),
            workspace_id: self.workspace_id.clone(),
            user_message: "test message".into(),
            source_agent_id: self.agent_id.clone(),
        }
    }

    fn make_strategies(
        &self,
        role: AgentRole,
        decision: AgentDecision,
    ) -> HashMap<AgentRole, Arc<dyn AgentStrategy>> {
        let mut map: HashMap<AgentRole, Arc<dyn AgentStrategy>> = HashMap::new();
        map.insert(role, Arc::new(FakeStrategy::new(role, decision)));
        map
    }

    fn make_task(&self, graph: &mut TaskGraph, role: AgentRole) -> crate::task_graph::AgentTask {
        let task_id = graph.add_task("test task", role, vec![]);
        graph.mark_running(&task_id).unwrap();
        graph.get_task(&task_id).cloned().unwrap()
    }

    async fn stored_events(&self) -> Vec<DomainEvent> {
        self.events
            .store
            .load_session(&self.session_id)
            .await
            .unwrap()
    }
}

// ---------------------------------------------------------------------------
// Tests for execute_task_with_strategy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn respond_branch_emits_assistant_message_completed() {
    let harness = TestHarness::new(vec![]).await;
    let mut graph = TaskGraph::default();
    let task = harness.make_task(&mut graph, AgentRole::Worker);
    let strategies = harness.make_strategies(
        AgentRole::Worker,
        AgentDecision::Respond("hello world".into()),
    );
    let ctx = harness.make_ctx();

    let result = execute_task_with_strategy(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &graph,
        &task,
        &[],
        &ctx,
        &harness.agent_id,
        None,
    )
    .await;

    assert!(result.is_ok());
    let events = harness.stored_events().await;
    assert_eq!(events.len(), 1);
    match &events[0].payload {
        EventPayload::AssistantMessageCompleted { content, .. } => {
            assert_eq!(content, "hello world");
        }
        other => panic!("Expected AssistantMessageCompleted, got {:?}", other),
    }
}

#[tokio::test]
async fn request_model_branch_streams_tokens_and_emits_events() {
    let harness = TestHarness::new(vec!["Hello".into(), " World".into()]).await;
    let mut graph = TaskGraph::default();
    let task = harness.make_task(&mut graph, AgentRole::Worker);
    let strategies = harness.make_strategies(
        AgentRole::Worker,
        AgentDecision::RequestModel { tools: vec![] },
    );
    let ctx = harness.make_ctx();

    let result = execute_task_with_strategy(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &graph,
        &task,
        &[],
        &ctx,
        &harness.agent_id,
        None,
    )
    .await;

    assert!(result.is_ok());
    let events = harness.stored_events().await;
    // 2 TokenDelta events + 1 AssistantMessageCompleted
    assert_eq!(events.len(), 3);
    match &events[0].payload {
        EventPayload::ModelTokenDelta { delta } => assert_eq!(delta, "Hello"),
        other => panic!("Expected ModelTokenDelta, got {:?}", other),
    }
    match &events[1].payload {
        EventPayload::ModelTokenDelta { delta } => assert_eq!(delta, " World"),
        other => panic!("Expected ModelTokenDelta, got {:?}", other),
    }
    match &events[2].payload {
        EventPayload::AssistantMessageCompleted { content, .. } => {
            assert_eq!(content, "Hello World");
        }
        other => panic!("Expected AssistantMessageCompleted, got {:?}", other),
    }
}

#[tokio::test]
async fn decompose_branch_returns_invalid_state_error() {
    let harness = TestHarness::new(vec![]).await;
    let mut graph = TaskGraph::default();
    let task = harness.make_task(&mut graph, AgentRole::Worker);
    let strategies = harness.make_strategies(
        AgentRole::Worker,
        AgentDecision::Decompose { sub_tasks: vec![] },
    );
    let ctx = harness.make_ctx();

    let result = execute_task_with_strategy(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &graph,
        &task,
        &[],
        &ctx,
        &harness.agent_id,
        None,
    )
    .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Nested decomposition"),
        "Error should mention nested decomposition, got: {err_msg}"
    );
}

#[tokio::test]
async fn review_complete_approved_emits_findings_and_returns_ok() {
    let harness = TestHarness::new(vec![]).await;
    let mut graph = TaskGraph::default();
    let task = harness.make_task(&mut graph, AgentRole::Reviewer);
    let findings = vec![crate::agents::ReviewerFinding {
        severity: "info".into(),
        message: "Looks good".into(),
    }];
    let strategies = harness.make_strategies(
        AgentRole::Reviewer,
        AgentDecision::ReviewComplete {
            approved: true,
            findings: findings.clone(),
        },
    );
    let ctx = harness.make_ctx();

    let result = execute_task_with_strategy(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &graph,
        &task,
        &[],
        &ctx,
        &harness.agent_id,
        None,
    )
    .await;

    assert!(result.is_ok());
    let events = harness.stored_events().await;
    assert_eq!(events.len(), 1);
    match &events[0].payload {
        EventPayload::ReviewerFindingAdded {
            severity, message, ..
        } => {
            assert_eq!(severity, "info");
            assert_eq!(message, "Looks good");
        }
        other => panic!("Expected ReviewerFindingAdded, got {:?}", other),
    }
}

#[tokio::test]
async fn review_complete_not_approved_returns_error() {
    let harness = TestHarness::new(vec![]).await;
    let mut graph = TaskGraph::default();
    let task = harness.make_task(&mut graph, AgentRole::Reviewer);
    let findings = vec![
        crate::agents::ReviewerFinding {
            severity: "high".into(),
            message: "Bug found".into(),
        },
        crate::agents::ReviewerFinding {
            severity: "medium".into(),
            message: "Style issue".into(),
        },
    ];
    let strategies = harness.make_strategies(
        AgentRole::Reviewer,
        AgentDecision::ReviewComplete {
            approved: false,
            findings,
        },
    );
    let ctx = harness.make_ctx();

    let result = execute_task_with_strategy(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &graph,
        &task,
        &[],
        &ctx,
        &harness.agent_id,
        None,
    )
    .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Review not approved"),
        "Expected review not approved error, got: {err_msg}"
    );
    // Findings should still have been emitted before the error
    let events = harness.stored_events().await;
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn cancellation_at_start_returns_cancelled_error() {
    let harness = TestHarness::new(vec![]).await;
    let mut graph = TaskGraph::default();
    let task = harness.make_task(&mut graph, AgentRole::Worker);
    let strategies =
        harness.make_strategies(AgentRole::Worker, AgentDecision::Respond("nope".into()));
    let ctx = harness.make_ctx();

    let token = CancellationToken::new();
    token.cancel();

    let result = execute_task_with_strategy(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &graph,
        &task,
        &[],
        &ctx,
        &harness.agent_id,
        Some(&token),
    )
    .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("cancelled"),
        "Expected cancelled error, got: {err_msg}"
    );
    // No events should have been emitted
    let events = harness.stored_events().await;
    assert!(events.is_empty());
}

#[tokio::test]
async fn unregistered_role_returns_invalid_state_error() {
    let harness = TestHarness::new(vec![]).await;
    let mut graph = TaskGraph::default();
    let task = harness.make_task(&mut graph, AgentRole::Worker);
    // Register strategy for Reviewer but task is Worker — mismatch
    let strategies = harness.make_strategies(
        AgentRole::Reviewer,
        AgentDecision::Respond("irrelevant".into()),
    );
    let ctx = harness.make_ctx();

    let result = execute_task_with_strategy(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &graph,
        &task,
        &[],
        &ctx,
        &harness.agent_id,
        None,
    )
    .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("No strategy registered for role"),
        "Expected no strategy error, got: {err_msg}"
    );
}

// ---------------------------------------------------------------------------
// Tests for run_reviewer_if_needed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_reviewer_no_reviewer_task_returns_ok() {
    let harness = TestHarness::new(vec![]).await;
    let mut graph = TaskGraph::default();
    // Only a Worker task, no Reviewer
    graph.add_task("worker task", AgentRole::Worker, vec![]);
    let strategies: HashMap<AgentRole, Arc<dyn AgentStrategy>> = HashMap::new();
    let ctx = harness.make_ctx();

    let result = run_reviewer_if_needed(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &mut graph,
        &[],
        &ctx,
        None,
    )
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn run_reviewer_dependencies_not_completed_skips_execution() {
    let harness = TestHarness::new(vec![]).await;
    let mut graph = TaskGraph::default();
    let worker_id = graph.add_task("worker task", AgentRole::Worker, vec![]);
    // Reviewer depends on worker which is still Pending (not Completed)
    let _reviewer_id = graph.add_task("review task", AgentRole::Reviewer, vec![worker_id]);
    let strategies = harness.make_strategies(
        AgentRole::Reviewer,
        AgentDecision::ReviewComplete {
            approved: true,
            findings: vec![],
        },
    );
    let ctx = harness.make_ctx();

    let result = run_reviewer_if_needed(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &mut graph,
        &[],
        &ctx,
        None,
    )
    .await;

    assert!(result.is_ok());
    // Reviewer should still be Pending since its dependency wasn't completed
    let snapshot = graph.snapshot();
    let reviewer = snapshot
        .iter()
        .find(|t| t.role == AgentRole::Reviewer)
        .unwrap();
    assert_eq!(reviewer.state, agent_core::TaskState::Pending);
}

#[tokio::test]
async fn run_reviewer_executes_successfully() {
    let harness = TestHarness::new(vec![]).await;
    let mut graph = TaskGraph::default();
    let worker_id = graph.add_task("worker task", AgentRole::Worker, vec![]);
    graph.mark_running(&worker_id).unwrap();
    graph.mark_completed(&worker_id).unwrap();
    let _reviewer_id = graph.add_task("review task", AgentRole::Reviewer, vec![worker_id]);
    let strategies = harness.make_strategies(
        AgentRole::Reviewer,
        AgentDecision::ReviewComplete {
            approved: true,
            findings: vec![],
        },
    );
    let ctx = harness.make_ctx();

    let result = run_reviewer_if_needed(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &mut graph,
        &[],
        &ctx,
        None,
    )
    .await;

    assert!(result.is_ok());
    let snapshot = graph.snapshot();
    let reviewer = snapshot
        .iter()
        .find(|t| t.role == AgentRole::Reviewer)
        .unwrap();
    assert_eq!(reviewer.state, agent_core::TaskState::Completed);
}

#[tokio::test]
async fn run_reviewer_execution_failure_marks_failed() {
    let harness = TestHarness::new(vec![]).await;
    let mut graph = TaskGraph::default();
    let worker_id = graph.add_task("worker task", AgentRole::Worker, vec![]);
    graph.mark_running(&worker_id).unwrap();
    graph.mark_completed(&worker_id).unwrap();
    let _reviewer_id = graph.add_task("review task", AgentRole::Reviewer, vec![worker_id]);
    // ReviewComplete with approved=false triggers an error inside execute_task_with_strategy
    let strategies = harness.make_strategies(
        AgentRole::Reviewer,
        AgentDecision::ReviewComplete {
            approved: false,
            findings: vec![crate::agents::ReviewerFinding {
                severity: "high".into(),
                message: "rejected".into(),
            }],
        },
    );
    let ctx = harness.make_ctx();

    let result = run_reviewer_if_needed(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &mut graph,
        &[],
        &ctx,
        None,
    )
    .await;

    // run_reviewer_if_needed returns Ok even when the reviewer fails — it marks_failed internally
    assert!(result.is_ok());
    let snapshot = graph.snapshot();
    let reviewer = snapshot
        .iter()
        .find(|t| t.role == AgentRole::Reviewer)
        .unwrap();
    assert_eq!(reviewer.state, agent_core::TaskState::Failed);
    assert!(reviewer
        .error
        .as_deref()
        .unwrap()
        .contains("Review not approved"));
}

#[tokio::test]
async fn run_reviewer_cancellation_before_execution_marks_cancelled() {
    let harness = TestHarness::new(vec![]).await;
    let mut graph = TaskGraph::default();
    let worker_id = graph.add_task("worker task", AgentRole::Worker, vec![]);
    graph.mark_running(&worker_id).unwrap();
    graph.mark_completed(&worker_id).unwrap();
    let _reviewer_id = graph.add_task("review task", AgentRole::Reviewer, vec![worker_id]);
    let strategies = harness.make_strategies(
        AgentRole::Reviewer,
        AgentDecision::ReviewComplete {
            approved: true,
            findings: vec![],
        },
    );
    let ctx = harness.make_ctx();

    let token = CancellationToken::new();
    token.cancel();

    let result = run_reviewer_if_needed(
        &harness.events,
        &harness.model,
        &strategies,
        &harness.permission_engine,
        &harness.model_config,
        &harness.workspace_id,
        &harness.session_id,
        &mut graph,
        &[],
        &ctx,
        Some(&token),
    )
    .await;

    assert!(result.is_ok());
    let snapshot = graph.snapshot();
    let reviewer = snapshot
        .iter()
        .find(|t| t.role == AgentRole::Reviewer)
        .unwrap();
    assert_eq!(reviewer.state, agent_core::TaskState::Cancelled);
}

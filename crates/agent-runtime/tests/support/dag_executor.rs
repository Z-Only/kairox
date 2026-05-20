#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use futures::{stream, stream::BoxStream};
use tokio::sync::Mutex;

use agent_core::{
    AgentRole, AppFacade, DomainEvent, EventPayload, PrivacyClassification, StartSessionRequest,
    WorkspaceId,
};
use agent_models::{
    FakeModelClient, ModelClient, ModelEvent, ModelMessage, ModelRequest, ToolCall,
};
use agent_runtime::{
    AgentDecision, AgentSettingsRoots, AgentStrategy, DagConfig, DagExecutor, LocalRuntime,
    StepContext, TaskGraph, ToolResultAction,
};
use agent_store::SqliteEventStore;
use agent_tools::{PermissionEngine, PermissionMode, ToolRegistry};

/// Build a DagExecutor with default strategies and an in-memory store.
pub async fn make_executor() -> DagExecutor<SqliteEventStore, FakeModelClient> {
    make_executor_with_config(DagConfig::default()).await
}

/// Build a DagExecutor with a custom config and an in-memory store.
pub async fn make_executor_with_config(
    config: DagConfig,
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
        config,
        AgentSettingsRoots::default(),
    )
    .await
}

/// Create a LocalRuntime wired with DAG execution enabled, plus workspace and session.
pub async fn make_runtime_with_session() -> (
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
pub struct FixedDecisionStrategy {
    role_val: AgentRole,
    decision: AgentDecision,
}

impl FixedDecisionStrategy {
    pub fn new(role_val: AgentRole, decision: AgentDecision) -> Self {
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
pub struct RecordingModelClient {
    requests: Arc<Mutex<Vec<ModelRequest>>>,
}

impl RecordingModelClient {
    pub fn new(requests: Arc<Mutex<Vec<ModelRequest>>>) -> Self {
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

/// Write an agent .md file with optional overrides into a scope directory.
#[allow(clippy::too_many_arguments)]
pub async fn write_agent_settings(
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
pub async fn make_executor_with_roots(
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

pub async fn append_model_profile_events(
    store: &SqliteEventStore,
    workspace_id: &WorkspaceId,
    session_id: &agent_core::SessionId,
) {
    use agent_store::EventStore;

    store
        .append(&DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            agent_core::AgentId::system(),
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
            agent_core::AgentId::system(),
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
}

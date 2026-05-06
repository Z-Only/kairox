use crate::dag_executor::{DagConfig, DagExecutor};
use crate::task_graph::TaskGraph;
use crate::McpServerManager;
use agent_core::{
    AgentStatusInfo, AppFacade, DomainEvent, PermissionDecision, SendMessageRequest, SessionId,
    StartSessionRequest, TaskId, TraceEntry, WorkspaceId, WorkspaceInfo,
};
use agent_mcp::types::McpServerDef;
use agent_memory::{ContextAssembler, MemoryStore};
use agent_store::EventStore;
use agent_tools::{BuiltinProvider, PermissionEngine, PermissionMode, ToolProvider, ToolRegistry};
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

const EVENT_CHANNEL_CAPACITY: usize = 1024;

/// Execution mode determines how the agent processes requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Default: current single-step agent loop behavior.
    SingleStep,
    /// DAG-driven: Planner decomposes, Workers execute in parallel, Reviewer evaluates.
    DagExecution,
}

pub struct LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    store: Arc<S>,
    model: Arc<M>,
    permission_engine: Arc<Mutex<PermissionEngine>>,
    mcp_manager: Option<Arc<Mutex<McpServerManager>>>,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    context_assembler: ContextAssembler,
    memory_store: Option<Arc<dyn MemoryStore>>,
    pending_permissions:
        Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    task_graphs: Arc<Mutex<HashMap<String, TaskGraph>>>,
    active_cancellation: Arc<Mutex<Option<CancellationToken>>>,
    dag_executor: Option<Arc<DagExecutor<S, M>>>,
    dag_config: DagConfig,
}

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub fn new(store: S, model: M) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            store: Arc::new(store),
            model: Arc::new(model),
            permission_engine: Arc::new(Mutex::new(PermissionEngine::new(PermissionMode::Suggest))),
            tool_registry: Arc::new(Mutex::new(ToolRegistry::new())),
            context_assembler: ContextAssembler::new_standalone(100_000),
            memory_store: None,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            task_graphs: Arc::new(Mutex::new(HashMap::new())),
            active_cancellation: Arc::new(Mutex::new(None)),
            mcp_manager: None,
            dag_executor: None,
            dag_config: DagConfig::default(),
        }
    }

    pub fn with_permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_engine = Arc::new(Mutex::new(PermissionEngine::new(mode)));
        self
    }

    pub fn with_context_limit(mut self, max_tokens: usize) -> Self {
        self.context_assembler = ContextAssembler::new_standalone(max_tokens);
        self
    }

    pub fn tool_registry(&self) -> Arc<Mutex<ToolRegistry>> {
        self.tool_registry.clone()
    }

    /// Get the current permission mode.
    pub async fn permission_mode(&self) -> PermissionMode {
        *self.permission_engine.lock().await.mode()
    }

    /// Set the memory store for persistent memory.
    pub fn with_memory_store(mut self, store: Arc<dyn MemoryStore>) -> Self {
        self.memory_store = Some(store.clone());
        self.context_assembler = ContextAssembler::new(100_000, store);
        self
    }

    /// Get a reference to the memory store (if configured).
    pub fn memory_store(&self) -> Option<Arc<dyn MemoryStore>> {
        self.memory_store.clone()
    }

    /// Register builtin tools (shell.exec, search.ripgrep, patch.apply, fs.read)
    pub async fn with_builtin_tools(self, workspace_root: PathBuf) -> Self {
        let provider = BuiltinProvider::with_defaults(workspace_root);
        self.tool_registry
            .lock()
            .await
            .add_provider(Box::new(provider))
            .await;
        self
    }

    /// Register a custom tool provider
    pub async fn with_provider(self, provider: Box<dyn ToolProvider>) -> Self {
        self.tool_registry.lock().await.add_provider(provider).await;
        self
    }

    /// Configure MCP servers from parsed config definitions.
    pub async fn with_mcp_servers(mut self, configs: Vec<McpServerDef>) -> Self {
        if configs.is_empty() {
            return self;
        }
        let mut manager = McpServerManager::from_config(
            configs,
            self.tool_registry.clone(),
            self.permission_engine.clone(),
            Some(self.event_tx.clone()),
        );
        let results = manager.start_persistent_servers().await;
        for result in &results {
            if let Err(e) = result {
                tracing::warn!("MCP server startup warning: {}", e);
            }
        }
        self.mcp_manager = Some(Arc::new(Mutex::new(manager)));
        self
    }

    /// Get a reference to the MCP server manager (if configured).
    pub fn mcp_manager(&self) -> Option<Arc<Mutex<McpServerManager>>> {
        self.mcp_manager.clone()
    }

    /// Enable DAG execution mode with the default configuration.
    pub fn with_dag_execution(mut self) -> Self {
        self.dag_config = DagConfig::default();
        self.dag_executor = Some(Arc::new(DagExecutor::new(
            self.store.clone(),
            self.model.clone(),
            self.event_tx.clone(),
            self.tool_registry.clone(),
            self.permission_engine.clone(),
            self.pending_permissions.clone(),
            self.memory_store.clone(),
            self.dag_config.clone(),
        )));
        self
    }

    /// Enable DAG execution mode with a custom configuration.
    pub fn with_dag_config(mut self, config: DagConfig) -> Self {
        self.dag_config = config.clone();
        self.dag_executor = Some(Arc::new(DagExecutor::new(
            self.store.clone(),
            self.model.clone(),
            self.event_tx.clone(),
            self.tool_registry.clone(),
            self.permission_engine.clone(),
            self.pending_permissions.clone(),
            self.memory_store.clone(),
            config,
        )));
        self
    }

    /// Determine the execution mode for a given request.
    fn execution_mode(&self, request: &SendMessageRequest) -> ExecutionMode {
        if request.content.starts_with("/plan ") && self.dag_executor.is_some() {
            ExecutionMode::DagExecution
        } else {
            ExecutionMode::SingleStep
        }
    }
}

/// Resolve a pending permission request (used by GUI Interactive mode).
impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub async fn resolve_permission(
        &self,
        request_id: &str,
        decision: PermissionDecision,
    ) -> agent_core::Result<()> {
        crate::permission::resolve_permission(&self.pending_permissions, request_id, decision).await
    }
}

#[async_trait]
impl<S, M> AppFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn open_workspace(&self, path: String) -> agent_core::Result<WorkspaceInfo> {
        crate::session::open_workspace(&*self.store, &self.event_tx, path).await
    }

    async fn start_session(&self, request: StartSessionRequest) -> agent_core::Result<SessionId> {
        crate::session::start_session(
            &*self.store,
            &self.event_tx,
            request.workspace_id,
            request.model_profile,
        )
        .await
    }

    async fn send_message(&self, request: SendMessageRequest) -> agent_core::Result<()> {
        match self.execution_mode(&request) {
            ExecutionMode::DagExecution => {
                let executor = self.dag_executor.as_ref().ok_or_else(|| {
                    agent_core::CoreError::InvalidState("DAG executor not available".into())
                })?;
                let result = executor.execute(&request, &self.task_graphs).await?;
                tracing::info!(
                    "DAG execution completed: {} tasks, {} completed, {} failed, {} skipped",
                    result.total_tasks,
                    result.completed,
                    result.failed,
                    result.skipped,
                );
                Ok(())
            }
            ExecutionMode::SingleStep => {
                crate::agent_loop::run_agent_loop(
                    &self.store,
                    &self.model,
                    &self.event_tx,
                    &self.tool_registry,
                    &self.permission_engine,
                    &self.pending_permissions,
                    &self.memory_store,
                    &self.task_graphs,
                    &self.active_cancellation,
                    &request,
                )
                .await
            }
        }
    }

    async fn decide_permission(&self, decision: PermissionDecision) -> agent_core::Result<()> {
        let _ = decision;
        Ok(())
    }

    async fn cancel_session(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> agent_core::Result<()> {
        crate::session::cancel_session(
            &*self.store,
            &self.event_tx,
            &self.active_cancellation,
            workspace_id,
            session_id,
        )
        .await
    }

    async fn get_session_projection(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<agent_core::projection::SessionProjection> {
        crate::session::get_session_projection(&*self.store, session_id).await
    }

    async fn get_trace(&self, session_id: SessionId) -> agent_core::Result<Vec<TraceEntry>> {
        crate::session::get_trace(&*self.store, session_id).await
    }

    fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, DomainEvent> {
        crate::session::subscribe_session(&self.event_tx, session_id)
    }

    fn subscribe_all(&self) -> BoxStream<'static, DomainEvent> {
        crate::session::subscribe_all(&self.event_tx)
    }

    async fn list_workspaces(&self) -> agent_core::Result<Vec<WorkspaceInfo>> {
        crate::session::list_workspaces(&*self.store).await
    }

    async fn list_sessions(
        &self,
        workspace_id: &WorkspaceId,
    ) -> agent_core::Result<Vec<agent_core::SessionMeta>> {
        crate::session::list_sessions(&*self.store, workspace_id).await
    }

    async fn rename_session(
        &self,
        session_id: &SessionId,
        title: String,
    ) -> agent_core::Result<()> {
        crate::session::rename_session(&*self.store, session_id, title).await
    }

    async fn soft_delete_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        crate::session::soft_delete_session(&*self.store, session_id).await
    }

    async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> agent_core::Result<usize> {
        crate::session::cleanup_expired_sessions(&*self.store, older_than).await
    }

    async fn get_task_graph(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<agent_core::TaskGraphSnapshot> {
        crate::session::get_task_graph(&self.task_graphs, session_id).await
    }

    async fn retry_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> agent_core::Result<()> {
        if let Some(executor) = &self.dag_executor {
            let mut graphs = self.task_graphs.lock().await;
            let graph = graphs.get_mut(&session_id.to_string()).ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!(
                    "No task graph found for session {}",
                    session_id
                ))
            })?;
            executor
                .retry_task(&workspace_id, &session_id, graph, &task_id)
                .await
        } else {
            Err(agent_core::CoreError::InvalidState(
                "DAG executor not available".into(),
            ))
        }
    }

    async fn cancel_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> agent_core::Result<()> {
        if let Some(executor) = &self.dag_executor {
            let mut graphs = self.task_graphs.lock().await;
            let graph = graphs.get_mut(&session_id.to_string()).ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!(
                    "No task graph found for session {}",
                    session_id
                ))
            })?;
            executor
                .cancel_task(&workspace_id, &session_id, graph, &task_id)
                .await
        } else {
            Err(agent_core::CoreError::InvalidState(
                "DAG executor not available".into(),
            ))
        }
    }

    async fn get_agent_status(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<Vec<AgentStatusInfo>> {
        let graphs = self.task_graphs.lock().await;
        match graphs.get(&session_id.to_string()) {
            Some(graph) => {
                if let Some(executor) = &self.dag_executor {
                    let statuses = executor.get_agent_status(graph);
                    Ok(statuses
                        .into_iter()
                        .map(|s| AgentStatusInfo {
                            agent_id: s.agent_id,
                            role: s.role,
                            task_id: s.task_id,
                            status: s.status,
                        })
                        .collect())
                } else {
                    Ok(Vec::new())
                }
            }
            None => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
    use agent_models::FakeModelClient;
    use agent_store::SqliteEventStore;

    #[tokio::test]
    async fn send_message_records_user_and_assistant_events() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hello".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hi".into(),
            })
            .await
            .unwrap();

        let projection = runtime.get_session_projection(session_id).await.unwrap();
        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].content, "hi");
        assert_eq!(projection.messages[1].content, "hello");
    }

    #[tokio::test]
    async fn open_workspace_persists_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();

        let workspaces = runtime.list_workspaces().await.unwrap();
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].workspace_id, workspace.workspace_id);
        assert_eq!(workspaces[0].path, "/tmp/project");
    }

    #[tokio::test]
    async fn start_session_persists_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, session_id);
        assert_eq!(sessions[0].title, "Session using fake");
    }

    #[tokio::test]
    async fn rename_session_updates_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .rename_session(&session_id, "My Custom Title".into())
            .await
            .unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert_eq!(sessions[0].title, "My Custom Title");
    }

    #[tokio::test]
    async fn soft_delete_hides_session() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime.soft_delete_session(&session_id).await.unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn default_execution_mode_is_single_step() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let request = SendMessageRequest {
            workspace_id: WorkspaceId::new(),
            session_id: SessionId::new(),
            content: "hello".into(),
        };
        assert_eq!(runtime.execution_mode(&request), ExecutionMode::SingleStep);
    }

    #[tokio::test]
    async fn plan_prefix_triggers_dag_mode() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_dag_execution();

        let request = SendMessageRequest {
            workspace_id: WorkspaceId::new(),
            session_id: SessionId::new(),
            content: "/plan implement feature X".into(),
        };
        assert_eq!(
            runtime.execution_mode(&request),
            ExecutionMode::DagExecution
        );
    }

    #[tokio::test]
    async fn no_plan_prefix_uses_single_step_even_with_dag() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_dag_execution();

        let request = SendMessageRequest {
            workspace_id: WorkspaceId::new(),
            session_id: SessionId::new(),
            content: "just a question".into(),
        };
        assert_eq!(runtime.execution_mode(&request), ExecutionMode::SingleStep);
    }
}

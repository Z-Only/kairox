use crate::dag_executor::DagExecutor;
use crate::execution_runtime::TurnExecutor;
use crate::facade_runtime::{ExecutionMode, LocalRuntime};
use crate::task_graph::TaskGraph;
use agent_core::{DomainEvent, PermissionDecision, SendMessageRequest, SessionId};
use agent_memory::MemoryStore;
use agent_models::ModelClient;
use agent_store::{EventStore, ProjectMetaRepository};
use agent_tools::{PermissionEngine, ToolRegistry};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub(crate) struct LocalRuntimeTurnExecutor<S, M>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    store: Arc<S>,
    model: Arc<M>,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    permission_engine: Arc<Mutex<PermissionEngine>>,
    pending_permissions:
        Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>,
    memory_store: Option<Arc<dyn MemoryStore>>,
    task_graphs: Arc<Mutex<HashMap<String, TaskGraph>>>,
    active_cancellation: Arc<Mutex<Option<CancellationToken>>>,
    dag_executor: Option<Arc<DagExecutor<S, M>>>,
    config: Arc<agent_config::Config>,
    session_states: Arc<Mutex<HashMap<String, crate::session::SessionState>>>,
    skill_registry: Option<Arc<dyn agent_skills::SkillRegistry>>,
    active_skills: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl<S, M> LocalRuntimeTurnExecutor<S, M>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    pub(crate) fn from_runtime(runtime: &LocalRuntime<S, M>) -> Self {
        Self {
            store: runtime.store.clone(),
            model: runtime.model.clone(),
            event_tx: runtime.event_tx.clone(),
            tool_registry: runtime.tool_registry.clone(),
            permission_engine: runtime.permission_engine.clone(),
            pending_permissions: runtime.pending_permissions.clone(),
            memory_store: runtime.memory_store.clone(),
            task_graphs: runtime.task_graphs.clone(),
            active_cancellation: runtime.active_cancellation.clone(),
            dag_executor: runtime.dag_executor.clone(),
            config: runtime.config.clone(),
            session_states: runtime.session_states.clone(),
            skill_registry: runtime.skill_registry.clone(),
            active_skills: runtime.active_skills.clone(),
        }
    }

    fn execution_mode(&self, request: &SendMessageRequest) -> ExecutionMode {
        if request.content.starts_with("/plan ") && self.dag_executor.is_some() {
            ExecutionMode::DagExecution
        } else {
            ExecutionMode::SingleStep
        }
    }

    async fn root_path_for_session(&self, session_id: &SessionId) -> Option<PathBuf> {
        let repository = self.store.sqlite_pool().map(ProjectMetaRepository::new)?;
        let binding = match repository.get_session_binding(session_id.as_str()).await {
            Ok(Some(binding)) => binding,
            _ => return None,
        };
        repository
            .get_project(&binding.project_id)
            .await
            .ok()
            .map(|project| PathBuf::from(project.root_path))
    }
}

#[async_trait]
impl<S, M> TurnExecutor for LocalRuntimeTurnExecutor<S, M>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    async fn execute_turn(
        &self,
        request: SendMessageRequest,
        cancellation: CancellationToken,
    ) -> agent_core::Result<()> {
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
                let root_path = self.root_path_for_session(&request.session_id).await;
                crate::agent_loop::run_agent_loop(
                    crate::agent_loop::AgentLoopDeps {
                        store: &self.store,
                        model: &self.model,
                        event_tx: &self.event_tx,
                        tool_registry: &self.tool_registry,
                        permission_engine: &self.permission_engine,
                        pending_permissions: &self.pending_permissions,
                        memory_store: &self.memory_store,
                        task_graphs: &self.task_graphs,
                        active_cancellation: &self.active_cancellation,
                        config: &self.config,
                        session_states: &self.session_states,
                        skill_registry: &self.skill_registry,
                        active_skills: &self.active_skills,
                        turn_cancellation: Some(cancellation),
                        root_path,
                    },
                    &request,
                )
                .await
            }
        }
    }
}

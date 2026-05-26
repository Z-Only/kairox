//! DAG-driven task executor — Phase 2 implementation.
//!
//! The `DagExecutor` takes a user goal, uses a `PlannerStrategy` to decompose
//! it into a `TaskGraph` (DAG), then schedules and executes tasks using the
//! appropriate `AgentStrategy` for each role. Tasks with satisfied dependencies
//! run in parallel up to `max_concurrency`. Failed tasks cascade `BlockDependents`
//! by default, with `retry_task()` and `skip_task()` for recovery.

pub(crate) mod agent_settings;
pub mod config;
pub(crate) mod events;
pub(crate) mod execution;
pub(crate) mod recovery;
pub(crate) mod scheduling;
pub mod types;

pub use config::DagConfig;
pub use types::{AgentStatus, ExecutionResult};

use agent_core::{
    AgentId, AgentRole, DomainEvent, EventPayload, PrivacyClassification, TaskId, TaskState,
    WorkspaceId,
};
use agent_memory::MemoryStore;
use agent_models::ModelClient;
use agent_store::EventStore;
use agent_tools::{PermissionEngine, ToolRegistry};
use events::EventEmitter;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::agent_settings::AgentSettingsRoots;
use crate::agents::{AgentDecision, AgentStrategy, StepContext};
use crate::task_graph::TaskGraph;

/// The DAG-driven task executor.
///
/// Orchestrates the Planner → Worker → Reviewer pipeline:
/// 1. Planner decomposes the user goal into sub-tasks
/// 2. Workers execute sub-tasks in parallel (bounded by semaphore)
/// 3. Reviewer evaluates the output
///
/// Execution is opt-in via the `/plan` prefix on user messages.
#[allow(dead_code)]
pub struct DagExecutor<S, M>
where
    S: EventStore,
    M: ModelClient,
{
    store: Arc<S>,
    model: Arc<M>,
    events: EventEmitter<S>,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    permission_engine: Arc<Mutex<PermissionEngine>>,
    memory_store: Option<Arc<dyn MemoryStore>>,
    config: DagConfig,
    agent_settings_roots: AgentSettingsRoots,
    strategies: HashMap<AgentRole, Arc<dyn AgentStrategy>>,
    pending_permissions:
        Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<agent_core::PermissionDecision>>>>,
}

impl<S, M> DagExecutor<S, M>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    /// Create a new DagExecutor.
    ///
    /// Loads effective agent settings from the provided roots and constructs
    /// strategies from matching agents. Falls back to hardcoded defaults for
    /// roles that have no matching effective agent.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        store: Arc<S>,
        model: Arc<M>,
        event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
        tool_registry: Arc<Mutex<ToolRegistry>>,
        permission_engine: Arc<Mutex<PermissionEngine>>,
        pending_permissions: Arc<
            Mutex<HashMap<String, tokio::sync::oneshot::Sender<agent_core::PermissionDecision>>>,
        >,
        memory_store: Option<Arc<dyn MemoryStore>>,
        config: DagConfig,
        agent_settings_roots: AgentSettingsRoots,
    ) -> Self {
        let agent_views = crate::agent_settings::list_agent_settings(agent_settings_roots.clone())
            .await
            .unwrap_or_default();

        let strategies = agent_settings::strategies_from_agent_settings(&agent_views);

        let events = EventEmitter {
            store: Arc::clone(&store),
            event_tx: event_tx.clone(),
        };

        Self {
            store,
            model,
            events,
            tool_registry,
            permission_engine,
            memory_store,
            config,
            agent_settings_roots,
            strategies,
            pending_permissions,
        }
    }

    /// Register a custom strategy for a role.
    pub fn with_strategy(mut self, role: AgentRole, strategy: Arc<dyn AgentStrategy>) -> Self {
        self.strategies.insert(role, strategy);
        self
    }

    /// Returns true if the executor has at least a planner strategy registered.
    pub fn is_available(&self) -> bool {
        self.strategies.contains_key(&AgentRole::Planner)
    }

    /// Get the current configuration.
    pub fn config(&self) -> &DagConfig {
        &self.config
    }

    /// Execute a user request through the DAG pipeline.
    ///
    /// This is the main entry point:
    /// 1. Create a root Planner task
    /// 2. Run the planner to decide: Decompose or Respond
    /// 3. If Decompose: build the task graph, schedule workers, run them
    /// 4. If all workers done: optionally run reviewer
    /// 5. Return the execution result
    pub async fn execute(
        &self,
        request: &agent_core::SendMessageRequest,
        task_graphs: &Arc<Mutex<HashMap<String, TaskGraph>>>,
    ) -> agent_core::Result<ExecutionResult> {
        // Step 1: Create root Planner task
        let root_title = if request.content.chars().count() > 50 {
            let truncated: String = request.content.chars().take(50).collect();
            format!("{truncated}...")
        } else {
            request.content.clone()
        };

        let mut graph = TaskGraph::default();
        let root_task_id = graph.add_task(&root_title, AgentRole::Planner, vec![]);
        graph.mark_running(&root_task_id).unwrap();

        // Emit root task events
        self.events
            .emit_task_created(
                &request.workspace_id,
                &request.session_id,
                &root_task_id,
                &root_title,
                AgentRole::Planner,
                &[],
            )
            .await?;
        self.events
            .emit_task_started(&request.workspace_id, &request.session_id, &root_task_id)
            .await?;

        // Emit agent spawned for planner
        let planner_agent_id = AgentId::planner();
        self.events
            .emit_agent_spawned(
                &request.workspace_id,
                &request.session_id,
                &planner_agent_id,
                AgentRole::Planner,
                &root_task_id,
            )
            .await?;

        // Step 2: Run the planner
        let ctx = StepContext {
            session_id: request.session_id.clone(),
            workspace_id: request.workspace_id.clone(),
            user_message: request.content.clone(),
            source_agent_id: planner_agent_id.clone(),
        };

        let planner = self.strategies.get(&AgentRole::Planner).ok_or_else(|| {
            agent_core::CoreError::InvalidState("No planner strategy registered".into())
        })?;

        // Load session history
        let session_events = self
            .store
            .load_session(&request.session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        let root_task = graph
            .get_task(&root_task_id)
            .cloned()
            .ok_or_else(|| agent_core::CoreError::InvalidState("Root task not found".into()))?;

        let messages = planner
            .build_context(&root_task, &graph, &session_events)
            .await;

        let decision = planner.decide(&ctx, messages).await;

        let result = match decision {
            AgentDecision::Decompose { sub_tasks } => {
                // Step 3: Build task graph from decomposition
                scheduling::handle_decomposition(
                    &self.events,
                    &self.config,
                    &request.workspace_id,
                    &request.session_id,
                    &root_task_id,
                    &sub_tasks,
                    &mut graph,
                )
                .await?;

                // Step 4: Run the scheduling loop
                scheduling::run_scheduling_loop(
                    &self.events,
                    &self.model,
                    &self.strategies,
                    &self.permission_engine,
                    &self.config,
                    &request.workspace_id,
                    &request.session_id,
                    &mut graph,
                    &session_events,
                    &ctx,
                )
                .await?;

                // Step 5: Run reviewer on completed worker outputs
                execution::run_reviewer_if_needed(
                    &self.events,
                    &self.model,
                    &self.strategies,
                    &self.permission_engine,
                    &request.workspace_id,
                    &request.session_id,
                    &mut graph,
                    &session_events,
                    &ctx,
                )
                .await?;

                // Mark root task as completed
                graph.mark_completed(&root_task_id).unwrap();
                self.events
                    .emit_task_completed(&request.workspace_id, &request.session_id, &root_task_id)
                    .await?;

                self.build_execution_result(&graph)
            }
            AgentDecision::Respond(text) => {
                // Planner decided to respond directly — single-step path
                let event = DomainEvent::new(
                    request.workspace_id.clone(),
                    request.session_id.clone(),
                    planner_agent_id,
                    PrivacyClassification::FullTrace,
                    EventPayload::AssistantMessageCompleted {
                        message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                        content: text,
                    },
                );
                crate::event_emitter::append_and_broadcast(
                    &*self.store,
                    &self.events.event_tx,
                    &event,
                )
                .await?;

                graph.mark_completed(&root_task_id).unwrap();
                self.events
                    .emit_task_completed(&request.workspace_id, &request.session_id, &root_task_id)
                    .await?;

                self.build_execution_result(&graph)
            }
            AgentDecision::RequestModel { .. } | AgentDecision::ReviewComplete { .. } => {
                // Planner wants to call the model directly — delegate to agent loop
                // For now, treat as a direct response
                graph.mark_completed(&root_task_id).unwrap();
                self.events
                    .emit_task_completed(&request.workspace_id, &request.session_id, &root_task_id)
                    .await?;

                self.build_execution_result(&graph)
            }
        };

        // Store the final graph
        task_graphs
            .lock()
            .await
            .insert(request.session_id.to_string(), graph.clone());

        // Emit AgentIdle for planner
        self.events
            .emit_agent_idle(
                &request.workspace_id,
                &request.session_id,
                &AgentId::planner(),
            )
            .await?;

        Ok(result)
    }

    /// Retry a previously failed task, resetting it to pending and unblocking dependents.
    pub async fn retry_task(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        graph: &mut TaskGraph,
        task_id: &TaskId,
    ) -> agent_core::Result<()> {
        recovery::retry_task(&self.events, workspace_id, session_id, graph, task_id).await
    }

    /// Cancel a specific task.
    pub async fn cancel_task(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        graph: &mut TaskGraph,
        task_id: &TaskId,
    ) -> agent_core::Result<()> {
        recovery::cancel_task(
            &self.events,
            &self.config,
            workspace_id,
            session_id,
            graph,
            task_id,
        )
        .await
    }

    /// Get the status of all agents associated with tasks in the graph.
    pub fn get_agent_status(&self, graph: &TaskGraph) -> Vec<AgentStatus> {
        graph
            .snapshot()
            .iter()
            .filter_map(|task| {
                task.assigned_agent_id.as_ref().map(|agent_id| AgentStatus {
                    agent_id: agent_id.to_string(),
                    role: task.role,
                    task_id: Some(task.id.clone()),
                    status: match task.state {
                        TaskState::Pending | TaskState::Ready => "idle".to_string(),
                        TaskState::Running => "running".to_string(),
                        TaskState::Completed => "completed".to_string(),
                        TaskState::Failed => "failed".to_string(),
                        TaskState::Blocked => "blocked".to_string(),
                        TaskState::Skipped => "skipped".to_string(),
                        TaskState::Cancelled => "cancelled".to_string(),
                    },
                })
            })
            .collect()
    }

    /// Return agent settings overrides for a role: (model_profile, skills, tools).
    #[doc(hidden)]
    #[allow(clippy::type_complexity)]
    pub fn agent_settings_overrides(
        &self,
        role: AgentRole,
    ) -> Option<(Option<String>, Vec<String>, Vec<String>)> {
        self.strategies.get(&role).map(|s| {
            (
                s.model_profile_override().map(String::from),
                s.skills().to_vec(),
                s.tools_allowlist().to_vec(),
            )
        })
    }

    fn build_execution_result(&self, graph: &TaskGraph) -> ExecutionResult {
        let counts = graph.state_counts();
        ExecutionResult {
            total_tasks: graph.snapshot().len(),
            completed: counts.completed,
            failed: counts.failed,
            skipped: counts.skipped,
            graph: graph.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
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
}

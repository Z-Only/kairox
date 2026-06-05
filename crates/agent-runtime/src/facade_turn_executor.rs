use crate::dag_executor::DagExecutor;
use crate::event_emitter::append_and_broadcast;
use crate::execution_runtime::{SessionExecutionRuntime, TaskControlExecutor, TurnExecutor};
use crate::facade_runtime::{ExecutionMode, LocalRuntime, RuntimeConfig};
use crate::task_graph::TaskGraph;
use agent_core::{
    AgentId, CompactionReason, CompactionSkipReason, DomainEvent, EventPayload,
    PrivacyClassification, SendMessageRequest, SessionId, TaskId, WorkspaceId,
};
use agent_memory::MemoryStore;
use agent_models::ModelClient;
use agent_store::{EventStore, ProjectMetaRepository, TrajectoryStore};
use agent_tools::{PermissionEngine, ToolRegistry, WorkspaceScopedBuiltinTools};
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
    pending_permissions: crate::permission::PendingPermissionsMap,
    memory_store: Option<Arc<dyn MemoryStore>>,
    task_graphs: Arc<Mutex<HashMap<String, TaskGraph>>>,
    dag_executor: Option<Arc<DagExecutor<S, M>>>,
    config: RuntimeConfig,
    session_states: Arc<Mutex<HashMap<String, crate::session::SessionState>>>,
    session_execution: SessionExecutionRuntime,
    skill_registry: Option<Arc<dyn agent_skills::SkillRegistry>>,
    skill_settings_roots: crate::skill_settings::SkillSettingsRoots,
    active_skills: Arc<Mutex<HashMap<String, Vec<String>>>>,
    workspace_scoped_builtin_tools: Option<Arc<WorkspaceScopedBuiltinTools>>,
    trajectory_store: Option<Arc<dyn TrajectoryStore>>,
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
            dag_executor: runtime.dag_executor.clone(),
            config: runtime.config.clone(),
            session_states: runtime.session_states.clone(),
            session_execution: runtime.session_execution.clone(),
            skill_registry: runtime.skill_registry.clone(),
            skill_settings_roots: runtime.skill_settings_roots.clone(),
            active_skills: runtime.active_skills.clone(),
            workspace_scoped_builtin_tools: runtime.workspace_scoped_builtin_tools.clone(),
            trajectory_store: runtime.trajectory_store.clone(),
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
        Some(PathBuf::from(binding.worktree_path))
    }

    /// Decide whether to enqueue an auto-compaction after a turn ends.
    /// Called from the tail of `execute_turn` for both `SingleStep` and
    /// `DagExecution`. The decision uses the same `should_trigger_auto_compaction`
    /// helper the turn-start trigger used to call.
    ///
    /// On `true`, spawns a detached task that calls
    /// `SessionExecutionRuntime::run_operation` so the actor serializes the
    /// compaction behind any user `RunTurn` already in the mailbox. On the
    /// two skip reasons callers care about (`AlreadyCompacting`,
    /// `ThresholdDisabled`), emits a `ContextCompactionSkipped` event so
    /// UIs can surface why a session that crossed the threshold did not
    /// compact. Below-threshold is the steady state and is silent.
    async fn maybe_schedule_auto_compaction(&self, request: &SendMessageRequest) {
        // 1. Read session state once.
        let (last_estimate, limits_opt, already_compacting) = {
            let states = self.session_states.lock().await;
            match states.get(&request.session_id.to_string()) {
                Some(s) => (
                    s.last_estimated_tokens,
                    s.model_limits.clone(),
                    s.compacting,
                ),
                None => return,
            }
        };
        let Some(limits) = limits_opt else { return };
        if last_estimate == 0 {
            return;
        }

        // 2. Reconstruct usage ratio from the same budget the turn used.
        let budget = crate::context_budget::build_budget(&limits);
        let usage = agent_core::ContextUsage {
            total_tokens: last_estimate,
            budget_tokens: budget.input_budget(),
            context_window: budget.context_window,
            output_reservation: budget.output_reservation,
            by_source: vec![],
            estimator: "cl100k_base".into(),
            corrected_by_real_usage: false,
        };
        let ratio = usage.ratio();
        let config = self.config.snapshot();
        let threshold = config.context.auto_compact_threshold;

        // 3. Decide.
        if crate::agent_loop::should_trigger_auto_compaction(&usage, threshold, already_compacting)
        {
            let session_id = request.session_id.clone();
            let workspace_id = request.workspace_id.clone();
            let profile_alias = self.resolve_compactor_profile(&session_id).await;
            let store = self.store.clone();
            let event_tx = self.event_tx.clone();
            let model = self.model.clone();
            let states_ref = self.session_states.clone();
            let rt = self.session_execution.clone();
            // Detached: do not block return to caller. The actor still
            // serializes the operation behind any in-flight or queued
            // `RunTurn`. The compactor emits its own
            // `ContextCompactionStarted/Completed/Failed` events.
            tokio::spawn(async move {
                let queued_id = session_id.clone();
                let result = rt
                    .run_operation(&queued_id, async move {
                        crate::compaction::compact_session(
                            &*store,
                            &event_tx,
                            &*model,
                            &profile_alias,
                            &states_ref,
                            workspace_id,
                            session_id,
                            CompactionReason::Threshold { ratio },
                        )
                        .await
                    })
                    .await;
                if let Err(e) = result {
                    tracing::warn!("auto-compaction enqueue failed for session {queued_id}: {e}");
                }
            });
            return;
        }

        // 4. Emit a skipped event for reasons callers care about.
        let skip_reason = if already_compacting {
            Some(CompactionSkipReason::AlreadyCompacting)
        } else if threshold >= 1.0 {
            Some(CompactionSkipReason::ThresholdDisabled)
        } else {
            None
        };
        if let Some(reason) = skip_reason {
            let event = DomainEvent::new(
                request.workspace_id.clone(),
                request.session_id.clone(),
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::ContextCompactionSkipped { reason, ratio },
            );
            if let Err(e) = append_and_broadcast(&*self.store, &self.event_tx, &event).await {
                tracing::warn!(
                    "failed to append ContextCompactionSkipped for session {}: {e}",
                    request.session_id
                );
            }
        }
    }

    /// Pick the model profile alias the compactor should use. Mirrors the
    /// inherent `LocalRuntime::compact_session` path: `compactor_profile`
    /// wins; otherwise use the session's latest profile, including any
    /// mid-session model switch.
    async fn resolve_compactor_profile(&self, session_id: &SessionId) -> String {
        let config = self.config.snapshot();
        if let Some(alias) = config.context.compactor_profile.clone() {
            return alias;
        }
        match self.store.load_session(session_id).await {
            Ok(events) => crate::agent_loop::latest_model_profile_for(&events),
            Err(_) => "fake".to_string(),
        }
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
        let outcome = match self.execution_mode(&request) {
            ExecutionMode::DagExecution => {
                let executor = self.dag_executor.as_ref().ok_or_else(|| {
                    agent_core::CoreError::InvalidState("DAG executor not available".into())
                })?;
                match executor
                    .execute_with_cancellation(&request, &self.task_graphs, cancellation)
                    .await
                {
                    Ok(result) => {
                        tracing::info!(
                            "DAG execution completed: {} tasks, {} completed, {} failed, {} skipped",
                            result.total_tasks,
                            result.completed,
                            result.failed,
                            result.skipped,
                        );
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            ExecutionMode::SingleStep => {
                let root_path = self.root_path_for_session(&request.session_id).await;
                let skill_registry = crate::skills::discover_skill_registry_for_settings_roots(
                    root_path
                        .as_deref()
                        .map(|root_path| {
                            crate::skills::skill_settings_roots_for_project_root(
                                self.skill_settings_roots.clone(),
                                root_path,
                            )
                        })
                        .unwrap_or_else(|| self.skill_settings_roots.clone()),
                    self.skill_registry.clone(),
                )
                .await?;
                let config = self.config.snapshot();
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
                        config: &config,
                        session_states: &self.session_states,
                        skill_registry: &skill_registry,
                        active_skills: &self.active_skills,
                        workspace_scoped_builtin_tools: &self.workspace_scoped_builtin_tools,
                        trajectory_store: &self.trajectory_store,
                        turn_cancellation: cancellation,
                        root_path,
                    },
                    &request,
                )
                .await
            }
        };

        if outcome.is_ok() {
            self.maybe_schedule_auto_compaction(&request).await;
        }
        outcome
    }
}

#[async_trait]
impl<S, M> TaskControlExecutor for LocalRuntimeTurnExecutor<S, M>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
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
}

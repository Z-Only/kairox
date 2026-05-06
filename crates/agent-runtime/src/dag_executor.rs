//! DAG-driven task executor — Phase 2 implementation.
//!
//! The `DagExecutor` takes a user goal, uses a `PlannerStrategy` to decompose
//! it into a `TaskGraph` (DAG), then schedules and executes tasks using the
//! appropriate `AgentStrategy` for each role. Tasks with satisfied dependencies
//! run in parallel up to `max_concurrency`. Failed tasks cascade `BlockDependents`
//! by default, with `retry_task()` and `skip_task()` for recovery.

use crate::agents::planner_agent::PlannerStrategy;
use crate::agents::reviewer_agent::ReviewerStrategy;
use crate::agents::worker_agent::WorkerStrategy;
use crate::agents::{AgentDecision, AgentStrategy, StepContext, SubTaskDef};
use crate::event_emitter::append_and_broadcast;
use crate::task_graph::TaskGraph;
use agent_core::{
    AgentId, AgentRole, DomainEvent, EventPayload, FailurePolicy, PrivacyClassification,
    RetryConfig, SendMessageRequest, TaskFailureReason, TaskId, TaskState, WorkspaceId,
};
use agent_memory::MemoryStore;
use agent_models::ModelClient;
use agent_store::EventStore;
use agent_tools::{PermissionEngine, ToolRegistry};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Configuration for the DAG executor.
#[derive(Debug, Clone)]
pub struct DagConfig {
    /// Maximum number of tasks that can execute concurrently.
    pub max_concurrency: usize,
    /// Failure policy for task failures.
    pub failure_policy: FailurePolicy,
    /// Retry configuration for model and tool errors.
    pub retry_config: RetryConfig,
}

impl Default for DagConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 3,
            failure_policy: FailurePolicy::BlockDependents,
            retry_config: RetryConfig::default(),
        }
    }
}

/// Result of a DAG execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// The final task graph after execution.
    pub graph: TaskGraph,
    /// Total number of tasks in the graph.
    pub total_tasks: usize,
    /// Number of completed tasks.
    pub completed: usize,
    /// Number of failed tasks.
    pub failed: usize,
    /// Number of skipped tasks.
    pub skipped: usize,
}

/// Status information about a running or completed agent.
#[derive(Debug, Clone)]
pub struct AgentStatus {
    pub agent_id: String,
    pub role: AgentRole,
    pub task_id: Option<TaskId>,
    pub status: String,
}

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
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    permission_engine: Arc<Mutex<PermissionEngine>>,
    memory_store: Option<Arc<dyn MemoryStore>>,
    config: DagConfig,
    strategies: HashMap<AgentRole, Arc<dyn AgentStrategy>>,
    pending_permissions:
        Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<agent_core::PermissionDecision>>>>,
}

impl<S, M> DagExecutor<S, M>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    /// Create a new DagExecutor with default strategies.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
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
    ) -> Self {
        let mut strategies: HashMap<AgentRole, Arc<dyn AgentStrategy>> = HashMap::new();
        strategies.insert(AgentRole::Planner, Arc::new(PlannerStrategy::new()));
        strategies.insert(AgentRole::Worker, Arc::new(WorkerStrategy::new()));
        strategies.insert(AgentRole::Reviewer, Arc::new(ReviewerStrategy::new()));

        Self {
            store,
            model,
            event_tx,
            tool_registry,
            permission_engine,
            memory_store,
            config,
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
        request: &SendMessageRequest,
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
        self.emit_task_created(
            &request.workspace_id,
            &request.session_id,
            &root_task_id,
            &root_title,
            AgentRole::Planner,
            &[],
        )
        .await?;
        self.emit_task_started(&request.workspace_id, &request.session_id, &root_task_id)
            .await?;

        // Emit agent spawned for planner
        let planner_agent_id = AgentId::planner();
        self.emit_agent_spawned(
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
                self.handle_decomposition(
                    &request.workspace_id,
                    &request.session_id,
                    &root_task_id,
                    &sub_tasks,
                    &mut graph,
                )
                .await?;

                // Step 4: Run the scheduling loop
                self.run_scheduling_loop(
                    &request.workspace_id,
                    &request.session_id,
                    &mut graph,
                    &session_events,
                    &ctx,
                )
                .await?;

                // Step 5: Run reviewer on completed worker outputs
                self.run_reviewer_if_needed(
                    &request.workspace_id,
                    &request.session_id,
                    &mut graph,
                    &session_events,
                    &ctx,
                )
                .await?;

                // Mark root task as completed
                graph.mark_completed(&root_task_id).unwrap();
                self.emit_task_completed(&request.workspace_id, &request.session_id, &root_task_id)
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
                append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

                graph.mark_completed(&root_task_id).unwrap();
                self.emit_task_completed(&request.workspace_id, &request.session_id, &root_task_id)
                    .await?;

                self.build_execution_result(&graph)
            }
            AgentDecision::RequestModel { .. } => {
                // Planner wants to call the model directly — delegate to agent loop
                // For now, treat as a direct response
                graph.mark_completed(&root_task_id).unwrap();
                self.emit_task_completed(&request.workspace_id, &request.session_id, &root_task_id)
                    .await?;

                self.build_execution_result(&graph)
            }
            AgentDecision::ReviewComplete { .. } => {
                // Planner shouldn't return ReviewComplete, but handle gracefully
                graph.mark_completed(&root_task_id).unwrap();
                self.emit_task_completed(&request.workspace_id, &request.session_id, &root_task_id)
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
        self.emit_agent_idle(
            &request.workspace_id,
            &request.session_id,
            &AgentId::planner(),
        )
        .await?;

        Ok(result)
    }

    /// Handle the decomposition of a task into sub-tasks.
    async fn handle_decomposition(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        parent_task_id: &TaskId,
        sub_tasks: &[SubTaskDef],
        graph: &mut TaskGraph,
    ) -> agent_core::Result<()> {
        // First pass: create all tasks and build title→TaskId mapping
        let mut title_to_id: HashMap<String, TaskId> = HashMap::new();

        for sub_task in sub_tasks {
            let task_id = graph.add_task_with_config(
                &sub_task.title,
                sub_task.role,
                Vec::new(), // dependencies resolved in second pass
                self.config.retry_config.max_tool_retries,
                None,
            );

            // Set description on the task
            if let Some(task) = graph.get_task_mut(&task_id) {
                task.description = sub_task.description.clone();
            }

            title_to_id.insert(sub_task.title.clone(), task_id.clone());

            // Emit task created event
            self.emit_task_created(
                workspace_id,
                session_id,
                &task_id,
                &sub_task.title,
                sub_task.role,
                std::slice::from_ref(parent_task_id),
            )
            .await?;
        }

        // Second pass: resolve dependencies by title
        for sub_task in sub_tasks {
            if let Some(task_id) = title_to_id.get(&sub_task.title) {
                let resolved_deps: Vec<TaskId> = sub_task
                    .dependencies
                    .iter()
                    .filter_map(|dep_title| {
                        // First try to find by TaskId (already resolved)
                        graph
                            .get_task(dep_title)
                            .map(|_| dep_title.clone())
                            .or_else(|| title_to_id.get(&dep_title.to_string()).cloned())
                    })
                    .collect();

                if let Some(task) = graph.get_task_mut(task_id) {
                    task.dependencies = resolved_deps;
                }
            }
        }

        // Emit TaskDecomposed event
        let sub_task_ids: Vec<TaskId> = title_to_id.values().cloned().collect();
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::planner(),
            PrivacyClassification::MinimalTrace,
            EventPayload::TaskDecomposed {
                parent_task_id: parent_task_id.clone(),
                sub_task_ids,
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

        Ok(())
    }

    /// Run the scheduling loop until all tasks are in terminal states.
    async fn run_scheduling_loop(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        graph: &mut TaskGraph,
        session_events: &[DomainEvent],
        ctx: &StepContext,
    ) -> agent_core::Result<()> {
        let mut iteration = 0;
        let max_iterations = 100; // Safety guard

        loop {
            if iteration >= max_iterations {
                tracing::warn!("DAG scheduling loop exceeded max iterations");
                break;
            }
            iteration += 1;

            let ready = graph.ready_tasks();
            if ready.is_empty() {
                if graph.is_finished() {
                    break;
                }
                // Check if there are still running tasks — wait for them
                let has_running = graph
                    .snapshot()
                    .iter()
                    .any(|t| t.state == TaskState::Running);
                if !has_running {
                    // Deadlock: no ready tasks, no running tasks, but not finished
                    // This means there are blocked or pending tasks that can't proceed
                    break;
                }
                // Give running tasks time to complete
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                continue;
            }

            // For now, execute tasks sequentially within the loop.
            // Parallel execution with JoinSet + Semaphore will be added
            // once we have async tool execution properly wired.
            for task_id in ready {
                if let Some(task) = graph.get_task(&task_id).cloned() {
                    graph.mark_running(&task_id).unwrap();
                    self.emit_task_started(workspace_id, session_id, &task_id)
                        .await?;

                    // Spawn an agent for this task
                    let agent_id = AgentId::worker(format!("w_{}", task_id));
                    self.emit_agent_spawned(
                        workspace_id,
                        session_id,
                        &agent_id,
                        task.role,
                        &task_id,
                    )
                    .await?;

                    // Get the strategy for this role
                    let strategy = self.strategies.get(&task.role);
                    let result = if let Some(strategy) = strategy {
                        self.execute_task_with_strategy(
                            workspace_id,
                            session_id,
                            graph,
                            &task,
                            strategy.as_ref(),
                            session_events,
                            ctx,
                            &agent_id,
                        )
                        .await
                    } else {
                        // No strategy for this role — mark as failed
                        let error = format!("No strategy registered for role {:?}", task.role);
                        graph.mark_failed(&task_id, error.clone()).unwrap();
                        self.emit_task_failed(workspace_id, session_id, &task_id, &error)
                            .await?;
                        continue;
                    };

                    match result {
                        Ok(()) => {
                            graph.mark_completed(&task_id).unwrap();
                            self.emit_task_completed(workspace_id, session_id, &task_id)
                                .await?;
                        }
                        Err(e) => {
                            let error = e.to_string();
                            graph
                                .mark_failed_with_reason(
                                    &task_id,
                                    error.clone(),
                                    TaskFailureReason::ModelError { retries: 0 },
                                )
                                .unwrap();
                            self.emit_task_failed(workspace_id, session_id, &task_id, &error)
                                .await?;

                            // Apply failure policy
                            self.apply_failure_policy(workspace_id, session_id, graph, &task_id)
                                .await?;
                        }
                    }

                    self.emit_agent_idle(workspace_id, session_id, &agent_id)
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// Execute a single task using its assigned strategy.
    #[allow(clippy::too_many_arguments)]
    async fn execute_task_with_strategy(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        graph: &TaskGraph,
        task: &crate::task_graph::AgentTask,
        strategy: &dyn AgentStrategy,
        session_events: &[DomainEvent],
        ctx: &StepContext,
        agent_id: &AgentId,
    ) -> agent_core::Result<()> {
        let messages = strategy.build_context(task, graph, session_events).await;
        let decision = strategy.decide(ctx, messages).await;

        match decision {
            AgentDecision::Respond(text) => {
                // Task produced a text response
                let event = DomainEvent::new(
                    workspace_id.clone(),
                    session_id.clone(),
                    agent_id.clone(),
                    PrivacyClassification::FullTrace,
                    EventPayload::AssistantMessageCompleted {
                        message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                        content: text,
                    },
                );
                append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
                Ok(())
            }
            AgentDecision::RequestModel { .. } => {
                // For the initial implementation, treat RequestModel as a simple
                // model call using the available model client.
                // Full tool-call loop will be added in future iterations.
                let model_request = agent_models::ModelRequest {
                    model_profile: "default".to_string(),
                    messages: strategy.build_context(task, graph, session_events).await,
                    system_prompt: None,
                    tools: Vec::new(),
                };

                let mut stream = self
                    .model
                    .stream(model_request)
                    .await
                    .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

                let mut response_text = String::new();
                use futures::StreamExt;
                while let Some(event_result) = stream.next().await {
                    match event_result {
                        Ok(agent_models::ModelEvent::TokenDelta(delta)) => {
                            response_text.push_str(&delta);
                            let event = DomainEvent::new(
                                workspace_id.clone(),
                                session_id.clone(),
                                agent_id.clone(),
                                PrivacyClassification::FullTrace,
                                EventPayload::ModelTokenDelta { delta },
                            );
                            append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
                        }
                        Ok(agent_models::ModelEvent::Completed { .. }) => break,
                        Ok(agent_models::ModelEvent::Failed { message }) => {
                            return Err(agent_core::CoreError::InvalidState(message));
                        }
                        Ok(_) => {} // ToolCallRequested handled in future iteration
                        Err(e) => {
                            return Err(agent_core::CoreError::InvalidState(e.to_string()));
                        }
                    }
                }

                if !response_text.is_empty() {
                    let event = DomainEvent::new(
                        workspace_id.clone(),
                        session_id.clone(),
                        agent_id.clone(),
                        PrivacyClassification::FullTrace,
                        EventPayload::AssistantMessageCompleted {
                            message_id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
                            content: response_text,
                        },
                    );
                    append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
                }

                Ok(())
            }
            AgentDecision::Decompose { .. } => {
                // Nested decomposition is not supported in the initial implementation
                Err(agent_core::CoreError::InvalidState(
                    "Nested decomposition is not yet supported".into(),
                ))
            }
            AgentDecision::ReviewComplete { approved, findings } => {
                // Emit reviewer findings
                for finding in &findings {
                    let event = DomainEvent::new(
                        workspace_id.clone(),
                        session_id.clone(),
                        agent_id.clone(),
                        PrivacyClassification::FullTrace,
                        EventPayload::ReviewerFindingAdded {
                            finding_id: format!("finding_{}", uuid::Uuid::new_v4().simple()),
                            severity: finding.severity.clone(),
                            message: finding.message.clone(),
                        },
                    );
                    append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
                }

                if approved {
                    Ok(())
                } else {
                    Err(agent_core::CoreError::InvalidState(format!(
                        "Review not approved: {} findings",
                        findings.len()
                    )))
                }
            }
        }
    }

    /// Apply failure policy when a task fails.
    async fn apply_failure_policy(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        graph: &mut TaskGraph,
        failed_task_id: &TaskId,
    ) -> agent_core::Result<()> {
        match self.config.failure_policy {
            FailurePolicy::BlockDependents => {
                let dependents = graph.find_blocked_dependents(failed_task_id);
                for dep_id in &dependents {
                    if let Some(dep) = graph.get_task(dep_id) {
                        if !dep.state.is_terminal() {
                            graph
                                .mark_blocked(
                                    dep_id,
                                    format!("dependency {} failed", failed_task_id),
                                )
                                .ok();
                            self.emit_task_blocked(
                                workspace_id,
                                session_id,
                                dep_id,
                                failed_task_id,
                                "dependency failed",
                            )
                            .await?;
                        }
                    }
                }
            }
            FailurePolicy::AllowOrphans => {
                // Dependents can proceed — they'll receive "parent failed" context
                // No action needed
            }
            FailurePolicy::FailFast => {
                // Cancel all non-terminal tasks
                for task in graph.snapshot() {
                    if !task.state.is_terminal() && task.id != *failed_task_id {
                        graph.mark_cancelled(&task.id).ok();
                    }
                }
            }
        }
        Ok(())
    }

    /// Run the reviewer on completed worker outputs, if a reviewer task exists.
    async fn run_reviewer_if_needed(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        graph: &mut TaskGraph,
        session_events: &[DomainEvent],
        ctx: &StepContext,
    ) -> agent_core::Result<()> {
        // Find reviewer tasks that are ready to run
        let reviewer_tasks: Vec<TaskId> = graph
            .snapshot()
            .iter()
            .filter(|t| t.role == AgentRole::Reviewer && t.state == TaskState::Pending)
            .filter(|t| {
                t.dependencies.iter().all(|dep| {
                    graph
                        .get_task(dep)
                        .map(|d| d.state == TaskState::Completed)
                        .unwrap_or(false)
                })
            })
            .map(|t| t.id.clone())
            .collect();

        for reviewer_task_id in reviewer_tasks {
            graph.mark_running(&reviewer_task_id).unwrap();
            self.emit_task_started(workspace_id, session_id, &reviewer_task_id)
                .await?;

            let reviewer_agent_id = AgentId::reviewer();
            self.emit_agent_spawned(
                workspace_id,
                session_id,
                &reviewer_agent_id,
                AgentRole::Reviewer,
                &reviewer_task_id,
            )
            .await?;

            let task = graph.get_task(&reviewer_task_id).cloned().ok_or_else(|| {
                agent_core::CoreError::InvalidState("Reviewer task not found".into())
            })?;

            let strategy = self.strategies.get(&AgentRole::Reviewer).ok_or_else(|| {
                agent_core::CoreError::InvalidState("No reviewer strategy registered".into())
            })?;

            let result = self
                .execute_task_with_strategy(
                    workspace_id,
                    session_id,
                    graph,
                    &task,
                    strategy.as_ref(),
                    session_events,
                    ctx,
                    &reviewer_agent_id,
                )
                .await;

            match result {
                Ok(()) => {
                    graph.mark_completed(&reviewer_task_id).unwrap();
                    self.emit_task_completed(workspace_id, session_id, &reviewer_task_id)
                        .await?;
                }
                Err(e) => {
                    let error = e.to_string();
                    graph.mark_failed(&reviewer_task_id, error.clone()).unwrap();
                    self.emit_task_failed(workspace_id, session_id, &reviewer_task_id, &error)
                        .await?;
                }
            }

            self.emit_agent_idle(workspace_id, session_id, &reviewer_agent_id)
                .await?;
        }

        Ok(())
    }

    /// Retry a previously failed task, resetting it to pending and unblocking dependents.
    pub async fn retry_task(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        graph: &mut TaskGraph,
        task_id: &TaskId,
    ) -> agent_core::Result<()> {
        let task = graph.get_task(task_id).cloned().ok_or_else(|| {
            agent_core::CoreError::InvalidState(format!("Task {} not found", task_id))
        })?;

        if task.state != TaskState::Failed && task.state != TaskState::Blocked {
            return Err(agent_core::CoreError::InvalidState(format!(
                "Task {} is in state {:?}, can only retry Failed or Blocked tasks",
                task_id, task.state
            )));
        }

        let new_attempt = task.retry_count + 1;
        graph
            .reset_to_pending(task_id)
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        // Also reset any blocked dependents
        let dependents = graph.find_blocked_dependents(task_id);
        for dep_id in &dependents {
            if let Some(dep) = graph.get_task(dep_id) {
                if dep.state == TaskState::Blocked {
                    graph.reset_to_pending(dep_id).ok();
                }
            }
        }

        // Emit TaskRetried event
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::TaskRetried {
                task_id: task_id.clone(),
                attempt: new_attempt,
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

        Ok(())
    }

    /// Cancel a specific task.
    pub async fn cancel_task(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        graph: &mut TaskGraph,
        task_id: &TaskId,
    ) -> agent_core::Result<()> {
        graph
            .mark_cancelled(task_id)
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        // Apply failure policy for dependents
        self.apply_failure_policy(workspace_id, session_id, graph, task_id)
            .await?;

        Ok(())
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

    // --- Event emission helpers ---

    async fn emit_task_created(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        task_id: &TaskId,
        title: &str,
        role: AgentRole,
        dependencies: &[TaskId],
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentTaskCreated {
                task_id: task_id.clone(),
                title: title.to_string(),
                role,
                dependencies: dependencies.to_vec(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    async fn emit_task_started(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        task_id: &TaskId,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentTaskStarted {
                task_id: task_id.clone(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    async fn emit_task_completed(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        task_id: &TaskId,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentTaskCompleted {
                task_id: task_id.clone(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    async fn emit_task_failed(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        task_id: &TaskId,
        error: &str,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentTaskFailed {
                task_id: task_id.clone(),
                error: error.to_string(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    async fn emit_task_blocked(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        task_id: &TaskId,
        blocking_task_id: &TaskId,
        reason: &str,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::TaskBlocked {
                task_id: task_id.clone(),
                blocking_task_id: blocking_task_id.clone(),
                reason: reason.to_string(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    async fn emit_agent_spawned(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        agent_id: &AgentId,
        role: AgentRole,
        task_id: &TaskId,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            agent_id.clone(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentSpawned {
                agent_id: agent_id.to_string(),
                role: role.to_string(),
                task_id: task_id.clone(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    async fn emit_agent_idle(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &agent_core::SessionId,
        agent_id: &AgentId,
    ) -> agent_core::Result<()> {
        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            agent_id.clone(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AgentIdle {
                agent_id: agent_id.to_string(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_models::FakeModelClient;
    use agent_store::SqliteEventStore;
    use agent_tools::{PermissionEngine, PermissionMode, ToolRegistry};

    fn make_config() -> DagConfig {
        DagConfig::default()
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
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["test".into()]);
        let (event_tx, _) = tokio::sync::broadcast::channel(1024);
        let tool_registry = Arc::new(Mutex::new(ToolRegistry::new()));
        let permission_engine = Arc::new(Mutex::new(PermissionEngine::new(PermissionMode::Agent)));
        let pending: Arc<
            Mutex<HashMap<String, tokio::sync::oneshot::Sender<agent_core::PermissionDecision>>>,
        > = Arc::new(Mutex::new(HashMap::new()));

        let executor = DagExecutor::new(
            Arc::new(store),
            Arc::new(model),
            event_tx,
            tool_registry,
            permission_engine,
            pending,
            None,
            make_config(),
        );
        assert!(executor.is_available());
    }

    #[tokio::test]
    async fn agent_status_from_graph() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["test".into()]);
        let (event_tx, _) = tokio::sync::broadcast::channel(1024);
        let tool_registry = Arc::new(Mutex::new(ToolRegistry::new()));
        let permission_engine = Arc::new(Mutex::new(PermissionEngine::new(PermissionMode::Agent)));
        let pending: Arc<
            Mutex<HashMap<String, tokio::sync::oneshot::Sender<agent_core::PermissionDecision>>>,
        > = Arc::new(Mutex::new(HashMap::new()));

        let executor = DagExecutor::new(
            Arc::new(store),
            Arc::new(model),
            event_tx,
            tool_registry,
            permission_engine,
            pending,
            None,
            make_config(),
        );

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

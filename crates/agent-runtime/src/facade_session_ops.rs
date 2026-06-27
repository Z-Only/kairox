use super::LocalRuntime;
use crate::execution_runtime::ExecutionState;
use crate::facade_turn_executor::LocalRuntimeTurnExecutor;
use agent_core::facade::SessionFacade;
use agent_core::{
    AgentStatusInfo, DomainEvent, PermissionDecision, SendMessageRequest, SessionId,
    StartSessionRequest, TaskConfirmationDecision, TaskId, TraceEntry, WorkspaceId, WorkspaceInfo,
};
use agent_store::EventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::sync::Arc;

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub async fn send_message_strict(&self, request: SendMessageRequest) -> agent_core::Result<()> {
        self.send_message_queued(request).await
    }

    pub async fn send_message_queued(&self, request: SendMessageRequest) -> agent_core::Result<()> {
        self.send_message_with_policy(request, true).await
    }

    pub async fn send_message_if_idle(
        &self,
        request: SendMessageRequest,
    ) -> agent_core::Result<()> {
        self.ensure_session_can_send(&request.session_id, true)
            .await?;
        self.mark_project_session_visible_for_request(&request)
            .await?;

        let executor = Arc::new(LocalRuntimeTurnExecutor::from_runtime(self));
        self.session_execution
            .run_turn_if_idle(request, executor)
            .await
    }

    pub async fn start_message_if_idle(
        &self,
        request: SendMessageRequest,
    ) -> agent_core::Result<()> {
        self.ensure_session_can_send(&request.session_id, true)
            .await?;
        self.mark_project_session_visible_for_request(&request)
            .await?;

        let executor = Arc::new(LocalRuntimeTurnExecutor::from_runtime(self));
        self.session_execution
            .start_turn_if_idle(request, executor)
            .await
    }

    pub async fn ensure_session_accepts_turn(
        &self,
        session_id: &SessionId,
    ) -> agent_core::Result<()> {
        self.ensure_session_can_send(session_id, true).await
    }

    async fn send_message_with_policy(
        &self,
        request: SendMessageRequest,
        reject_active_execution: bool,
    ) -> agent_core::Result<()> {
        self.ensure_session_can_send(&request.session_id, reject_active_execution)
            .await?;
        self.mark_project_session_visible_for_request(&request)
            .await?;

        let executor = Arc::new(LocalRuntimeTurnExecutor::from_runtime(self));
        self.session_execution.run_turn(request, executor).await
    }

    async fn ensure_session_can_send(
        &self,
        session_id: &SessionId,
        reject_active_execution: bool,
    ) -> agent_core::Result<()> {
        {
            let states = self.session_states.lock().await;
            if let Some(entry) = states.get(&session_id.to_string()) {
                if entry.compacting {
                    return Err(agent_core::CoreError::SessionBusy {
                        session_id: session_id.to_string(),
                        reason: "context compaction in progress".into(),
                    });
                }
            }
        }

        if reject_active_execution {
            match self.session_execution.session_state(session_id).await {
                Some(ExecutionState::Cancelling { turn_id }) => {
                    return Err(agent_core::CoreError::SessionBusy {
                        session_id: session_id.to_string(),
                        reason: format!("session execution cancelling ({turn_id})"),
                    });
                }
                Some(
                    ExecutionState::Idle | ExecutionState::Running { .. } | ExecutionState::Stopped,
                )
                | None => {}
            }
        }

        Ok(())
    }

    async fn mark_project_session_visible_for_request(
        &self,
        request: &SendMessageRequest,
    ) -> agent_core::Result<()> {
        if let Ok(repository) = self.project_repository() {
            if let Ok(Some(_binding)) = repository
                .get_session_binding(request.session_id.as_str())
                .await
            {
                let visibility = repository
                    .get_session_visibility(request.session_id.as_str())
                    .await
                    .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
                if visibility.as_deref() == Some("draft_hidden") {
                    self.mark_session_visible(
                        &request.session_id,
                        request
                            .display_content
                            .clone()
                            .unwrap_or_else(|| request.content.clone()),
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl<S, M> SessionFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn open_workspace(&self, path: String) -> agent_core::Result<WorkspaceInfo> {
        let workspace_path = path.clone();
        let info = crate::session::open_workspace(&*self.store, &self.event_tx, path).await?;
        let root_uri = crate::lsp_manager::file_uri_from_path(&workspace_path);
        self.start_lsp_servers(&root_uri).await;
        Ok(info)
    }

    async fn start_session(&self, request: StartSessionRequest) -> agent_core::Result<SessionId> {
        let workspace_id = request.workspace_id.clone();
        let model_profile_alias = request.model_profile.clone();
        let approval_policy_str = request.approval_policy.clone();
        let sandbox_policy_str = request.sandbox_policy.clone();
        let session_id = crate::session::start_session(
            &*self.store,
            &self.event_tx,
            request.workspace_id,
            request.model_profile,
            request.approval_policy,
            request.sandbox_policy,
        )
        .await?;

        self.initialize_session_limits(&session_id, &model_profile_alias)
            .await;
        self.session_execution.ensure_session(&session_id).await;

        if let Some(ref approval_str) = approval_policy_str {
            if let Ok(approval) = approval_str.parse::<ApprovalPolicy>() {
                self.permission_engine
                    .lock()
                    .await
                    .set_approval_policy(approval);
            }
        }
        if let Some(ref sandbox_str) = sandbox_policy_str {
            if let Ok(sandbox) = serde_json::from_str::<SandboxPolicy>(sandbox_str) {
                self.permission_engine
                    .lock()
                    .await
                    .set_sandbox_policy(sandbox);
            }
        }

        let config = self.config();
        crate::hooks::run_hooks_logged(
            &config,
            agent_config::HookEvent::SessionStart,
            "*",
            None,
            serde_json::json!({
                "workspace_id": workspace_id,
                "session_id": session_id,
                "model_profile": model_profile_alias,
            }),
        )
        .await;

        Ok(session_id)
    }

    async fn send_message(&self, request: SendMessageRequest) -> agent_core::Result<()> {
        self.send_message_with_policy(request, false).await
    }

    async fn decide_permission(&self, decision: PermissionDecision) -> agent_core::Result<()> {
        let request_id = decision.request_id.clone();
        crate::permission::resolve_permission(&self.pending_permissions, &request_id, decision)
            .await
    }

    async fn decide_task_confirmation(
        &self,
        decision: TaskConfirmationDecision,
    ) -> agent_core::Result<()> {
        crate::task_confirmation::resolve_task_confirmation(
            &self.pending_task_confirmations,
            decision,
        )
        .await
    }

    async fn cancel_session(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> agent_core::Result<()> {
        if let Some(registry) = &self.monitor_registry {
            registry.stop_all().await;
        }
        self.session_execution
            .cancel_session(&session_id, "user requested cancellation".into())
            .await?;
        crate::permission::deny_pending_permissions_for_session(
            &self.pending_permissions,
            &session_id,
            "cancelled by user",
        )
        .await?;
        crate::task_confirmation::deny_pending_confirmations_for_session(
            &self.pending_task_confirmations,
            &session_id,
            "cancelled by user",
        )
        .await?;
        crate::session::cancel_session(&*self.store, &self.event_tx, workspace_id, session_id).await
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
        if let Some(registry) = &self.monitor_registry {
            registry.stop_all().await;
        }
        self.session_execution.shutdown_session(session_id).await?;
        crate::session::soft_delete_session(&*self.store, session_id).await
    }

    async fn permanently_delete_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        if let Some(registry) = &self.monitor_registry {
            registry.stop_all().await;
        }
        self.session_execution.shutdown_session(session_id).await?;
        crate::session::permanently_delete_session(&*self.store, session_id.as_str()).await
    }

    async fn restore_archived_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        crate::session::restore_archived_session(&*self.store, session_id.as_str()).await?;
        self.session_execution.ensure_session(session_id).await;
        Ok(())
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
        let executor = Arc::new(LocalRuntimeTurnExecutor::from_runtime(self));
        self.session_execution
            .retry_task(workspace_id, session_id, task_id, executor)
            .await
    }

    async fn cancel_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> agent_core::Result<()> {
        let executor = Arc::new(LocalRuntimeTurnExecutor::from_runtime(self));
        self.session_execution
            .cancel_task(workspace_id, session_id, task_id, executor)
            .await
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

    async fn list_trajectories(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<Vec<agent_core::TrajectoryMeta>> {
        match self.trajectory_store.as_ref() {
            Some(ts) => ts
                .list_by_session(session_id.as_str())
                .await
                .map_err(|e| agent_core::CoreError::InvalidState(e.to_string())),
            None => Ok(vec![]),
        }
    }

    async fn get_trajectory_steps(
        &self,
        trajectory_id: agent_core::TrajectoryId,
    ) -> agent_core::Result<Vec<agent_core::TrajectoryStep>> {
        match self.trajectory_store.as_ref() {
            Some(ts) => ts
                .load_steps(&trajectory_id)
                .await
                .map_err(|e| agent_core::CoreError::InvalidState(e.to_string())),
            None => Ok(vec![]),
        }
    }

    async fn export_trajectory(
        &self,
        trajectory_id: agent_core::TrajectoryId,
    ) -> agent_core::Result<serde_json::Value> {
        match self.trajectory_store.as_ref() {
            Some(ts) => ts
                .export_json(&trajectory_id)
                .await
                .map_err(|e| agent_core::CoreError::InvalidState(e.to_string())),
            None => Ok(serde_json::json!(null)),
        }
    }
}

#[cfg(test)]
#[path = "facade_session_ops_tests.rs"]
mod tests;

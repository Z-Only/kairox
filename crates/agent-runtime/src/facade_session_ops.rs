use super::LocalRuntime;
use crate::facade_turn_executor::LocalRuntimeTurnExecutor;
use agent_core::facade::SessionFacade;
use agent_core::{
    AgentStatusInfo, DomainEvent, PermissionDecision, SendMessageRequest, SessionId,
    StartSessionRequest, TaskId, TraceEntry, WorkspaceId, WorkspaceInfo,
};
use agent_store::EventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::sync::Arc;

#[async_trait]
impl<S, M> SessionFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn open_workspace(&self, path: String) -> agent_core::Result<WorkspaceInfo> {
        crate::session::open_workspace(&*self.store, &self.event_tx, path).await
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

        crate::hooks::run_hooks_logged(
            &self.config,
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
        // Reject sends while a compaction is in flight (P2 busy gate).
        // The state is cleared by `compaction::compact_session` on exit.
        {
            let states = self.session_states.lock().await;
            if let Some(entry) = states.get(&request.session_id.to_string()) {
                if entry.compacting {
                    return Err(agent_core::CoreError::SessionBusy {
                        session_id: request.session_id.to_string(),
                        reason: "context compaction in progress".into(),
                    });
                }
            }
        }

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
                    self.mark_session_visible(&request.session_id, request.content.clone())
                        .await?;
                }
            }
        }

        let executor = Arc::new(LocalRuntimeTurnExecutor::from_runtime(self));
        self.session_execution.run_turn(request, executor).await
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
        self.session_execution
            .cancel_session(&session_id, "user requested cancellation".into())
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
        self.session_execution.shutdown_session(session_id).await?;
        crate::session::soft_delete_session(&*self.store, session_id).await
    }

    async fn permanently_delete_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        self.session_execution.shutdown_session(session_id).await?;
        crate::session::permanently_delete_session(&*self.store, session_id.as_str()).await
    }

    async fn restore_archived_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        crate::session::restore_archived_session(&*self.store, session_id.as_str()).await
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
}

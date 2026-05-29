use super::*;
use async_trait::async_trait;
use futures::stream::BoxStream;

fn assert_facade_is_object_safe(_: &dyn AppFacade) {}

struct NoopFacade;

#[async_trait]
impl SessionFacade for NoopFacade {
    async fn open_workspace(&self, path: String) -> crate::Result<WorkspaceInfo> {
        Ok(WorkspaceInfo {
            workspace_id: WorkspaceId::new(),
            path,
        })
    }

    async fn start_session(&self, request: StartSessionRequest) -> crate::Result<SessionId> {
        let _ = request;
        Ok(SessionId::new())
    }

    async fn send_message(&self, request: SendMessageRequest) -> crate::Result<()> {
        let _ = request;
        Ok(())
    }

    async fn decide_permission(&self, decision: PermissionDecision) -> crate::Result<()> {
        let _ = decision;
        Ok(())
    }

    async fn cancel_session(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> crate::Result<()> {
        let _ = (workspace_id, session_id);
        Ok(())
    }

    async fn get_session_projection(
        &self,
        session_id: SessionId,
    ) -> crate::Result<crate::projection::SessionProjection> {
        let _ = session_id;
        Ok(crate::projection::SessionProjection::default())
    }

    async fn get_trace(&self, session_id: SessionId) -> crate::Result<Vec<TraceEntry>> {
        let _ = session_id;
        Ok(Vec::new())
    }

    fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, crate::DomainEvent> {
        let _ = session_id;
        Box::pin(futures::stream::empty())
    }

    fn subscribe_all(&self) -> BoxStream<'static, crate::DomainEvent> {
        Box::pin(futures::stream::empty())
    }

    async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceInfo>> {
        Ok(Vec::new())
    }

    async fn list_sessions(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<SessionMeta>> {
        let _ = workspace_id;
        Ok(Vec::new())
    }

    async fn rename_session(&self, session_id: &SessionId, title: String) -> crate::Result<()> {
        let _ = (session_id, title);
        Ok(())
    }

    async fn soft_delete_session(&self, session_id: &SessionId) -> crate::Result<()> {
        let _ = session_id;
        Ok(())
    }

    async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> crate::Result<usize> {
        let _ = older_than;
        Ok(0)
    }

    async fn get_task_graph(&self, session_id: SessionId) -> crate::Result<TaskGraphSnapshot> {
        let _ = session_id;
        Ok(TaskGraphSnapshot::default())
    }

    async fn retry_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> crate::Result<()> {
        let _ = (workspace_id, session_id, task_id);
        Ok(())
    }

    async fn cancel_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> crate::Result<()> {
        let _ = (workspace_id, session_id, task_id);
        Ok(())
    }

    async fn get_agent_status(&self, session_id: SessionId) -> crate::Result<Vec<AgentStatusInfo>> {
        let _ = session_id;
        Ok(Vec::new())
    }
}

#[async_trait]
impl SkillsFacade for NoopFacade {}

#[async_trait]
impl McpFacade for NoopFacade {}

#[async_trait]
impl ProjectFacade for NoopFacade {}

#[async_trait]
impl AgentsFacade for NoopFacade {}

#[async_trait]
impl PluginsFacade for NoopFacade {}

impl AppFacade for NoopFacade {}

#[test]
fn facade_is_object_safe() {
    let facade = NoopFacade;
    assert_facade_is_object_safe(&facade);
}

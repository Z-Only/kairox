//! Session management sub-trait — required operations every runtime must implement.

use crate::projection::SessionProjection;
use crate::{
    AgentStatusInfo, DomainEvent, PermissionDecision, SendMessageRequest, SessionId, SessionMeta,
    StartSessionRequest, TaskGraphSnapshot, TaskId, TraceEntry, WorkspaceId, WorkspaceInfo,
};
use async_trait::async_trait;
use futures::stream::BoxStream;

#[async_trait]
pub trait SessionFacade: Send + Sync {
    async fn open_workspace(&self, path: String) -> crate::Result<WorkspaceInfo>;
    async fn start_session(&self, request: StartSessionRequest) -> crate::Result<SessionId>;
    async fn send_message(&self, request: SendMessageRequest) -> crate::Result<()>;
    async fn decide_permission(&self, decision: PermissionDecision) -> crate::Result<()>;
    async fn cancel_session(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> crate::Result<()>;
    async fn get_session_projection(
        &self,
        session_id: SessionId,
    ) -> crate::Result<SessionProjection>;
    async fn get_trace(&self, session_id: SessionId) -> crate::Result<Vec<TraceEntry>>;
    fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, DomainEvent>;
    fn subscribe_all(&self) -> BoxStream<'static, DomainEvent>;
    async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceInfo>>;
    async fn list_sessions(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<SessionMeta>>;
    async fn rename_session(&self, session_id: &SessionId, title: String) -> crate::Result<()>;
    async fn soft_delete_session(&self, session_id: &SessionId) -> crate::Result<()>;
    async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> crate::Result<usize>;
    async fn get_task_graph(&self, session_id: SessionId) -> crate::Result<TaskGraphSnapshot>;
    async fn retry_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> crate::Result<()>;
    async fn cancel_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> crate::Result<()>;
    async fn get_agent_status(&self, session_id: SessionId) -> crate::Result<Vec<AgentStatusInfo>>;
}

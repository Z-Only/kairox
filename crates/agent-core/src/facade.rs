use crate::{DomainEvent, SessionId, WorkspaceId};
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub workspace_id: WorkspaceId,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartSessionRequest {
    pub workspace_id: WorkspaceId,
    pub model_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub request_id: String,
    pub approve: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEntry {
    pub event: DomainEvent,
}

#[async_trait]
pub trait AppFacade: Send + Sync {
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
    ) -> crate::Result<crate::projection::SessionProjection>;
    async fn get_trace(&self, session_id: SessionId) -> crate::Result<Vec<TraceEntry>>;
    fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, DomainEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_facade_is_object_safe(_: &dyn AppFacade) {}

    struct NoopFacade;

    #[async_trait]
    impl AppFacade for NoopFacade {
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

        fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, DomainEvent> {
            let _ = session_id;
            Box::pin(futures::stream::empty())
        }
    }

    #[test]
    fn facade_is_object_safe() {
        let facade = NoopFacade;
        assert_facade_is_object_safe(&facade);
    }
}

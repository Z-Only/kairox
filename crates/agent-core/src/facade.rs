//! Application facade — the primary integration point for Kairox.
//!
//! All UIs (TUI, GUI) interact with the runtime through the [`AppFacade`] trait.
//! This trait provides a stable, object-safe interface for workspace management,
//! session lifecycle, messaging, permissions, and event streaming.

use crate::{DomainEvent, SessionId, TaskId, WorkspaceId};
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Workspace metadata returned after opening a workspace.
pub struct WorkspaceInfo {
    pub workspace_id: WorkspaceId,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Request to start a new agent session within a workspace.
pub struct StartSessionRequest {
    pub workspace_id: WorkspaceId,
    pub model_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Request to send a user message to an active session.
pub struct SendMessageRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// User decision on a permission request (approve or deny).
pub struct PermissionDecision {
    pub request_id: String,
    pub approve: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// A single trace entry wrapping a domain event, used for trace panel display.
pub struct TraceEntry {
    pub event: DomainEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Metadata for a session, used for listing and display.
pub struct SessionMeta {
    pub session_id: SessionId,
    pub workspace_id: WorkspaceId,
    pub title: String,
    pub model_profile: String,
    pub model_id: Option<String>,
    pub provider: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
/// A snapshot of a single task in the task graph.
pub struct TaskSnapshot {
    pub id: TaskId,
    pub title: String,
    pub role: crate::AgentRole,
    pub state: crate::TaskState,
    pub dependencies: Vec<TaskId>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
/// A snapshot of the entire task graph for a session.
pub struct TaskGraphSnapshot {
    pub tasks: Vec<TaskSnapshot>,
}

#[async_trait]
/// The primary integration point for Kairox.
///
/// All user interfaces (TUI, GUI) interact with the runtime through this trait.
/// The canonical implementation is [`crate::LocalRuntime`](agent_runtime::LocalRuntime),
/// but any mock or test implementation can substitute.
///
/// # Object Safety
///
/// This trait is object-safe and can be used as `dyn AppFacade`.
pub trait AppFacade: Send + Sync {
    /// Open a workspace at the given filesystem path. Returns workspace metadata.
    async fn open_workspace(&self, path: String) -> crate::Result<WorkspaceInfo>;
    /// Start a new agent session within a workspace using the specified model profile.
    async fn start_session(&self, request: StartSessionRequest) -> crate::Result<SessionId>;
    /// Send a user message to an active session. The agent loop runs in the background.
    async fn send_message(&self, request: SendMessageRequest) -> crate::Result<()>;
    /// Submit a user decision on a pending permission request.
    async fn decide_permission(&self, decision: PermissionDecision) -> crate::Result<()>;
    /// Cancel a running session.
    async fn cancel_session(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> crate::Result<()>;
    /// Get the projected (rolled-up) state of a session, including messages and task titles.
    async fn get_session_projection(
        &self,
        session_id: SessionId,
    ) -> crate::Result<crate::projection::SessionProjection>;
    /// Get the full trace of domain events for a session.
    async fn get_trace(&self, session_id: SessionId) -> crate::Result<Vec<TraceEntry>>;
    /// Subscribe to a real-time stream of domain events for a session.
    fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, DomainEvent>;
    /// Subscribe to a real-time stream of all domain events across all sessions.
    fn subscribe_all(&self) -> BoxStream<'static, DomainEvent>;
    /// List all workspaces known to the runtime.
    async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceInfo>>;
    /// List all sessions in a workspace, including soft-deleted ones.
    async fn list_sessions(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<SessionMeta>>;
    /// Rename a session.
    async fn rename_session(&self, session_id: &SessionId, title: String) -> crate::Result<()>;
    /// Soft-delete a session (marks as deleted without removing data).
    async fn soft_delete_session(&self, session_id: &SessionId) -> crate::Result<()>;
    /// Clean up sessions that were soft-deleted longer than the specified duration ago.
    async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> crate::Result<usize>;
    /// Get the current task graph snapshot for a session.
    async fn get_task_graph(&self, session_id: SessionId) -> crate::Result<TaskGraphSnapshot>;
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

        /// Cancel a running session.
        async fn cancel_session(
            &self,
            workspace_id: WorkspaceId,
            session_id: SessionId,
        ) -> crate::Result<()> {
            let _ = (workspace_id, session_id);
            Ok(())
        }

        /// Get the projected (rolled-up) state of a session, including messages and task titles.
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

        fn subscribe_all(&self) -> BoxStream<'static, DomainEvent> {
            Box::pin(futures::stream::empty())
        }

        async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceInfo>> {
            Ok(Vec::new())
        }

        async fn list_sessions(
            &self,
            workspace_id: &WorkspaceId,
        ) -> crate::Result<Vec<SessionMeta>> {
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
    }

    #[test]
    fn facade_is_object_safe() {
        let facade = NoopFacade;
        assert_facade_is_object_safe(&facade);
    }
}

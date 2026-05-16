//! Session DTOs and management sub-trait.

use crate::projection::SessionProjection;
use crate::{DomainEvent, ProjectId, SessionId, TaskFailureReason, TaskId, TaskState, WorkspaceId};
use serde::{Deserialize, Serialize};

use super::ProjectSessionVisibility;
use async_trait::async_trait;
use futures::stream::BoxStream;

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
    pub permission_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
/// Metadata for a file attached to a user message.
pub struct AttachmentInfo {
    /// Absolute filesystem path.
    pub path: String,
    /// Display filename.
    pub name: String,
    /// MIME type (e.g. "image/png", "application/pdf").
    pub mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Request to send a user message to an active session.
pub struct SendMessageRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub content: String,
    pub attachments: Vec<AttachmentInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// User decision on a permission request (approve or deny).
pub struct PermissionDecision {
    pub request_id: String,
    pub approve: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// A single trace entry wrapping a domain event, used for trace panel display.
///
/// Note: only `PartialEq` (not `Eq`) because the wrapped `DomainEvent::payload`
/// contains `f32` fields (`ContextUsage`, `CompactionReason::Threshold { ratio }`).
pub struct TraceEntry {
    pub event: DomainEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Metadata for a session, used for listing and display.
pub struct SessionMeta {
    pub project_id: Option<ProjectId>,
    pub worktree_path: Option<String>,
    pub branch: Option<String>,
    pub visibility: Option<ProjectSessionVisibility>,
    pub permission_mode: Option<String>,
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
    pub state: TaskState,
    pub dependencies: Vec<TaskId>,
    pub error: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub retry_count: usize,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub max_retries: usize,
    pub assigned_agent_id: Option<String>,
    pub failure_reason: Option<TaskFailureReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
/// A snapshot of the entire task graph for a session.
pub struct TaskGraphSnapshot {
    pub tasks: Vec<TaskSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
/// Status information about a running or completed agent.
pub struct AgentStatusInfo {
    pub agent_id: String,
    pub role: crate::AgentRole,
    pub task_id: Option<TaskId>,
    pub status: String,
}

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
    async fn permanently_delete_session(&self, session_id: &SessionId) -> crate::Result<()> {
        let _ = session_id;
        Ok(())
    }
    async fn restore_archived_session(&self, session_id: &SessionId) -> crate::Result<()> {
        let _ = session_id;
        Ok(())
    }
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

#[cfg(test)]
mod task_snapshot_tests {
    use super::*;
    use crate::{AgentRole, TaskId};

    #[test]
    fn task_snapshot_field_access() {
        let snapshot = TaskSnapshot {
            id: TaskId::new(),
            title: "review PR #42".into(),
            role: AgentRole::Reviewer,
            state: TaskState::Pending,
            dependencies: vec![],
            error: None,
            retry_count: 0,
            max_retries: 3,
            assigned_agent_id: None,
            failure_reason: None,
        };

        // Verify fields hold the values we set.
        assert_eq!(snapshot.title, "review PR #42");
        assert_eq!(snapshot.role, AgentRole::Reviewer);
        assert_eq!(snapshot.state, TaskState::Pending);
        assert!(snapshot.dependencies.is_empty());
        assert!(snapshot.error.is_none());
        assert_eq!(snapshot.retry_count, 0);
        assert_eq!(snapshot.max_retries, 3);
        assert!(snapshot.assigned_agent_id.is_none());
        assert!(snapshot.failure_reason.is_none());
    }

    #[test]
    fn task_snapshot_with_error_and_failure_reason() {
        let failure = TaskFailureReason::ToolExhausted {
            tool_id: "shell.exec".into(),
            attempts: 3,
            last_error: "command not found".into(),
        };
        let snapshot = TaskSnapshot {
            id: TaskId::new(),
            title: "run tests".into(),
            role: AgentRole::Worker,
            state: TaskState::Failed,
            dependencies: vec![],
            error: Some("max retries exceeded".into()),
            retry_count: 3,
            max_retries: 3,
            assigned_agent_id: Some("agent_worker_test".into()),
            failure_reason: Some(failure.clone()),
        };

        assert_eq!(snapshot.state, TaskState::Failed);
        assert_eq!(snapshot.error.as_deref(), Some("max retries exceeded"));
        assert_eq!(snapshot.retry_count, 3);
        assert_eq!(
            snapshot.assigned_agent_id.as_deref(),
            Some("agent_worker_test")
        );
        assert_eq!(snapshot.failure_reason, Some(failure));
    }

    #[test]
    fn task_graph_snapshot_contains_tasks() {
        let task1 = TaskSnapshot {
            id: TaskId::new(),
            title: "plan architecture".into(),
            role: AgentRole::Planner,
            state: TaskState::Completed,
            dependencies: vec![],
            error: None,
            retry_count: 0,
            max_retries: 3,
            assigned_agent_id: Some("agent_planner".into()),
            failure_reason: None,
        };
        let task2 = TaskSnapshot {
            id: TaskId::new(),
            title: "implement feature".into(),
            role: AgentRole::Worker,
            state: TaskState::Running,
            dependencies: vec![task1.id.clone()],
            error: None,
            retry_count: 0,
            max_retries: 3,
            assigned_agent_id: Some("agent_worker_impl".into()),
            failure_reason: None,
        };

        let graph = TaskGraphSnapshot {
            tasks: vec![task1.clone(), task2.clone()],
        };

        assert_eq!(graph.tasks.len(), 2);
        assert!(graph.tasks.contains(&task1));
        assert!(graph.tasks.contains(&task2));

        // task2 depends on task1.
        assert_eq!(graph.tasks[1].dependencies, vec![task1.id.clone()]);
    }

    #[test]
    fn task_graph_snapshot_serializes_roundtrip() {
        let task = TaskSnapshot {
            id: TaskId::new(),
            title: "verify".into(),
            role: AgentRole::Reviewer,
            state: TaskState::Completed,
            dependencies: vec![],
            error: None,
            retry_count: 0,
            max_retries: 3,
            assigned_agent_id: None,
            failure_reason: None,
        };
        let graph = TaskGraphSnapshot { tasks: vec![task] };

        let json = serde_json::to_string(&graph).unwrap();
        let back: TaskGraphSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(graph, back);
    }
}

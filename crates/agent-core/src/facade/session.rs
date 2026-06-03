//! Session DTOs and management sub-trait.

use crate::projection::SessionProjection;
use crate::{DomainEvent, ProjectId, SessionId, TaskFailureReason, TaskId, TaskState, WorkspaceId};
use chrono::{DateTime, Utc};
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
    /// Approval policy (`never` | `on_request` | `always`).
    #[serde(default)]
    pub approval_policy: Option<String>,
    /// Sandbox policy serialized as JSON
    /// (`{"kind":"read_only"}` | `{"kind":"workspace_write",...}` |
    /// `{"kind":"danger_full_access"}`). See `SandboxPolicy` in `agent-tools`.
    #[serde(default)]
    pub sandbox_policy: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_content: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
/// Structured trace export envelope for diagnostics and replay tooling.
pub struct TraceExport {
    pub schema_version: u32,
    pub session_id: SessionId,
    pub generated_at: DateTime<Utc>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub event_count: usize,
    pub events: Vec<DomainEvent>,
}

impl TraceExport {
    pub fn new(session_id: SessionId, events: Vec<DomainEvent>) -> Self {
        let event_count = events.len();
        Self {
            schema_version: 1,
            session_id,
            generated_at: Utc::now(),
            event_count,
            events,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Metadata for a session, used for listing and display.
pub struct SessionMeta {
    pub project_id: Option<ProjectId>,
    pub worktree_path: Option<String>,
    pub branch: Option<String>,
    pub visibility: Option<ProjectSessionVisibility>,
    /// Persisted approval policy for this session.
    #[serde(default)]
    pub approval_policy: Option<String>,
    /// Persisted sandbox policy JSON for this session.
    #[serde(default)]
    pub sandbox_policy: Option<String>,
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
    async fn export_trace(&self, session_id: SessionId) -> crate::Result<TraceExport> {
        let trace = self.get_trace(session_id.clone()).await?;
        let events = trace.into_iter().map(|entry| entry.event).collect();
        Ok(TraceExport::new(session_id, events))
    }
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
#[path = "session_tests.rs"]
mod tests;

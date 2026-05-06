use crate::ids::{AgentId, SessionId, TaskId, WorkspaceId};
use crate::AgentRole;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClassification {
    MinimalTrace,
    FullTrace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum EventPayload {
    WorkspaceOpened {
        path: String,
    },
    SessionInitialized {
        model_profile: String,
    },
    UserMessageAdded {
        message_id: String,
        content: String,
    },
    AgentTaskCreated {
        task_id: TaskId,
        title: String,
        role: AgentRole,
        dependencies: Vec<TaskId>,
    },
    AgentTaskStarted {
        task_id: TaskId,
    },
    ContextAssembled {
        token_estimate: usize,
        sources: Vec<String>,
    },
    ModelRequestStarted {
        model_profile: String,
        model_id: String,
    },
    ModelTokenDelta {
        delta: String,
    },
    ModelToolCallRequested {
        tool_call_id: String,
        tool_id: String,
    },
    PermissionRequested {
        request_id: String,
        tool_id: String,
        preview: String,
    },
    PermissionGranted {
        request_id: String,
    },
    PermissionDenied {
        request_id: String,
        reason: String,
    },
    ToolInvocationStarted {
        invocation_id: String,
        tool_id: String,
    },
    ToolInvocationCompleted {
        invocation_id: String,
        tool_id: String,
        output_preview: String,
        exit_code: Option<i32>,
        duration_ms: u64,
        truncated: bool,
    },
    ToolInvocationFailed {
        invocation_id: String,
        tool_id: String,
        error: String,
    },
    FilePatchProposed {
        patch_id: String,
        diff: String,
    },
    FilePatchApplied {
        patch_id: String,
    },
    MemoryProposed {
        memory_id: String,
        scope: String,
        key: Option<String>,
        content: String,
    },
    MemoryAccepted {
        memory_id: String,
        scope: String,
        key: Option<String>,
        content: String,
    },
    MemoryRejected {
        memory_id: String,
        reason: String,
    },
    ReviewerFindingAdded {
        finding_id: String,
        severity: String,
        message: String,
    },
    AssistantMessageCompleted {
        message_id: String,
        content: String,
    },
    AgentTaskCompleted {
        task_id: TaskId,
    },
    AgentTaskFailed {
        task_id: TaskId,
        error: String,
    },
    TaskDecomposed {
        parent_task_id: TaskId,
        sub_task_ids: Vec<TaskId>,
    },
    TaskBlocked {
        task_id: TaskId,
        blocking_task_id: TaskId,
        reason: String,
    },
    AgentSpawned {
        agent_id: String,
        role: String,
        task_id: TaskId,
    },
    AgentIdle {
        agent_id: String,
    },
    TaskRetried {
        task_id: TaskId,
        attempt: usize,
    },
    SessionCancelled {
        reason: String,
    },
    McpServerStarting {
        server_id: String,
    },
    McpServerReady {
        server_id: String,
        tool_count: usize,
    },
    McpServerStopped {
        server_id: String,
    },
    McpServerFailed {
        server_id: String,
        error: String,
    },
    McpToolCallStarted {
        server_id: String,
        tool_name: String,
    },
    McpToolCallCompleted {
        server_id: String,
        tool_name: String,
        duration_ms: u64,
    },
    McpTrustGranted {
        server_id: String,
    },
    McpTrustRevoked {
        server_id: String,
    },
    CatalogRefreshed {
        source: String,
        entry_count: usize,
    },
    CatalogEntryInstalling {
        catalog_id: String,
        source: String,
    },
    CatalogEntryInstalled {
        catalog_id: String,
        source: String,
        server_id: String,
    },
    CatalogEntryUninstalled {
        server_id: String,
    },
    CatalogRuntimeMissing {
        catalog_id: String,
        missing: Vec<String>,
    },
}

impl EventPayload {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::WorkspaceOpened { .. } => "WorkspaceOpened",
            Self::SessionInitialized { .. } => "SessionInitialized",
            Self::UserMessageAdded { .. } => "UserMessageAdded",
            Self::AgentTaskCreated { .. } => "AgentTaskCreated",
            Self::AgentTaskStarted { .. } => "AgentTaskStarted",
            Self::ContextAssembled { .. } => "ContextAssembled",
            Self::ModelRequestStarted { .. } => "ModelRequestStarted",
            Self::ModelTokenDelta { .. } => "ModelTokenDelta",
            Self::ModelToolCallRequested { .. } => "ModelToolCallRequested",
            Self::PermissionRequested { .. } => "PermissionRequested",
            Self::PermissionGranted { .. } => "PermissionGranted",
            Self::PermissionDenied { .. } => "PermissionDenied",
            Self::ToolInvocationStarted { .. } => "ToolInvocationStarted",
            Self::ToolInvocationCompleted { .. } => "ToolInvocationCompleted",
            Self::ToolInvocationFailed { .. } => "ToolInvocationFailed",
            Self::FilePatchProposed { .. } => "FilePatchProposed",
            Self::FilePatchApplied { .. } => "FilePatchApplied",
            Self::MemoryProposed { .. } => "MemoryProposed",
            Self::MemoryAccepted { .. } => "MemoryAccepted",
            Self::MemoryRejected { .. } => "MemoryRejected",
            Self::ReviewerFindingAdded { .. } => "ReviewerFindingAdded",
            Self::AssistantMessageCompleted { .. } => "AssistantMessageCompleted",
            Self::AgentTaskCompleted { .. } => "AgentTaskCompleted",
            Self::AgentTaskFailed { .. } => "AgentTaskFailed",
            Self::TaskDecomposed { .. } => "TaskDecomposed",
            Self::TaskBlocked { .. } => "TaskBlocked",
            Self::AgentSpawned { .. } => "AgentSpawned",
            Self::AgentIdle { .. } => "AgentIdle",
            Self::TaskRetried { .. } => "TaskRetried",
            Self::SessionCancelled { .. } => "SessionCancelled",
            Self::McpServerStarting { .. } => "McpServerStarting",
            Self::McpServerReady { .. } => "McpServerReady",
            Self::McpServerStopped { .. } => "McpServerStopped",
            Self::McpServerFailed { .. } => "McpServerFailed",
            Self::McpToolCallStarted { .. } => "McpToolCallStarted",
            Self::McpToolCallCompleted { .. } => "McpToolCallCompleted",
            Self::McpTrustGranted { .. } => "McpTrustGranted",
            Self::McpTrustRevoked { .. } => "McpTrustRevoked",
            Self::CatalogRefreshed { .. } => "CatalogRefreshed",
            Self::CatalogEntryInstalling { .. } => "CatalogEntryInstalling",
            Self::CatalogEntryInstalled { .. } => "CatalogEntryInstalled",
            Self::CatalogEntryUninstalled { .. } => "CatalogEntryUninstalled",
            Self::CatalogRuntimeMissing { .. } => "CatalogRuntimeMissing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DomainEvent {
    pub schema_version: u32,
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub timestamp: DateTime<Utc>,
    pub source_agent_id: AgentId,
    pub privacy: PrivacyClassification,
    pub event_type: String,
    pub payload: EventPayload,
}

impl DomainEvent {
    pub fn new(
        workspace_id: WorkspaceId,
        session_id: SessionId,
        source_agent_id: AgentId,
        privacy: PrivacyClassification,
        payload: EventPayload,
    ) -> Self {
        let event_type = payload.event_type().to_string();
        Self {
            schema_version: 1,
            workspace_id,
            session_id,
            timestamp: Utc::now(),
            source_agent_id,
            privacy,
            event_type,
            payload,
        }
    }

    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{AgentId, SessionId, WorkspaceId};
    use chrono::TimeZone;

    #[test]
    fn serializes_user_message_event_with_required_envelope_fields() {
        let event = DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::UserMessageAdded {
                message_id: "msg-user-1".into(),
                content: "explain the repo".into(),
            },
        )
        .with_timestamp(chrono::Utc.with_ymd_and_hms(2026, 4, 29, 2, 0, 0).unwrap());

        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["event_type"], "UserMessageAdded");
        assert_eq!(json["privacy"], "full_trace");
        assert_eq!(json["timestamp"], "2026-04-29T02:00:00Z");
        assert_eq!(json["source_agent_id"], "agent_system");
        assert_eq!(json["payload"]["content"], "explain the repo");
        assert!(json["workspace_id"].as_str().unwrap().starts_with("wrk_"));
        assert!(json["session_id"].as_str().unwrap().starts_with("ses_"));
    }
}

// MCP event tests
#[test]
fn mcp_server_starting_serializes() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpServerStarting {
            server_id: "test".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpServerStarting");
    assert_eq!(json["payload"]["server_id"], "test");
}

#[test]
fn mcp_server_ready_serializes() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpServerReady {
            server_id: "fs".into(),
            tool_count: 5,
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpServerReady");
    assert_eq!(json["payload"]["tool_count"], 5);
}

#[test]
fn mcp_server_failed_serializes() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpServerFailed {
            server_id: "bad".into(),
            error: "crashed".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpServerFailed");
    assert_eq!(json["payload"]["error"], "crashed");
}

#[test]
fn mcp_tool_call_completed_serializes() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpToolCallCompleted {
            server_id: "github".into(),
            tool_name: "create_issue".into(),
            duration_ms: 150,
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpToolCallCompleted");
    assert_eq!(json["payload"]["duration_ms"], 150);
}

#[test]
fn mcp_trust_events_serialize() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpTrustGranted {
            server_id: "fs".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpTrustGranted");

    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::McpTrustRevoked {
            server_id: "fs".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["type"], "McpTrustRevoked");
}

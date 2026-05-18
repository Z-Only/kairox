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

/// Why a session compaction was triggered. `Threshold { ratio }` is fired
/// automatically by `agent_loop` when `ContextAssembled.usage.ratio()`
/// crosses `ContextPolicy.auto_compact_threshold`. `UserRequested` is the
/// manual path (TUI `:compact` / GUI button — both wired in P3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type")]
pub enum CompactionReason {
    UserRequested,
    Threshold { ratio: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        usage: crate::context_types::ContextUsage,
    },
    ContextCompactionStarted {
        reason: CompactionReason,
        #[cfg_attr(feature = "specta", specta(type = u32))]
        before_tokens: u64,
        #[cfg_attr(feature = "specta", specta(type = u32))]
        candidate_event_count: usize,
    },
    ContextCompactionCompleted {
        summary_id: String,
        #[cfg_attr(feature = "specta", specta(type = u32))]
        after_tokens: u64,
        fallback_used: bool,
    },
    ContextCompactionFailed {
        error: String,
        fallback_used: bool,
    },
    CompactionSummary {
        summary_id: String,
        content: String,
        replaces_event_range: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
        reason: CompactionReason,
        #[cfg_attr(feature = "specta", specta(type = u32))]
        before_tokens: u64,
        #[cfg_attr(feature = "specta", specta(type = u32))]
        after_tokens: u64,
        summarised_by_profile: String,
    },
    /// Mid-session model profile change. The new profile only takes effect
    /// at the next `send_message` (agent-loop entry) — in-flight streams
    /// continue on the old profile end-to-end so provider-specific
    /// tool-call formats don't get mixed mid-stream.
    ModelProfileSwitched {
        from_profile: String,
        to_profile: String,
        #[serde(default)]
        reasoning_effort: Option<String>,
        effective_at: DateTime<Utc>,
        /// Mirrors `agent_models::ModelLimits.context_window` so this
        /// event can be consumed by `agent-core` projections without
        /// introducing a cycle on `agent-models`.
        #[cfg_attr(feature = "specta", specta(type = u32))]
        context_window: u64,
        /// Mirrors `agent_models::ModelLimits.output_limit`.
        #[cfg_attr(feature = "specta", specta(type = u32))]
        output_limit: u64,
        /// Snake-case `agent_models::LimitSource` discriminant: one of
        /// `"user_config" | "builtin_registry" | "runtime_probe" | "fallback"`.
        limit_source: String,
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
        #[cfg_attr(feature = "specta", specta(type = u32))]
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
        #[cfg_attr(feature = "specta", specta(type = u32))]
        attempt: usize,
    },
    TaskCancelled {
        task_id: TaskId,
    },
    SessionCancelled {
        reason: String,
    },
    SkillDiscovered {
        skill_id: String,
        name: String,
        source: String,
    },
    SkillValidationFailed {
        path: String,
        error: String,
    },
    SkillActivated {
        skill_id: String,
        name: String,
        source: String,
        activation_mode: String,
    },
    SkillDeactivated {
        skill_id: String,
        name: String,
        source: String,
    },
    SkillSuggested {
        skill_id: String,
        name: String,
        reason: String,
    },
    McpServerStarting {
        server_id: String,
    },
    McpServerReady {
        server_id: String,
        #[cfg_attr(feature = "specta", specta(type = u32))]
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
        #[cfg_attr(feature = "specta", specta(type = u32))]
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
        #[cfg_attr(feature = "specta", specta(type = u32))]
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
    CatalogSourceAdded {
        source: String,
    },
    CatalogSourceFailed {
        source: String,
        error: String,
    },
    /// Emitted incrementally as each catalog source completes its query.
    /// `entries` is the current state of the fully-merged, sorted list
    /// across all providers that have responded so far.
    CatalogSourceResultsArrived {
        source: String,
        entries: Vec<crate::facade::ServerEntry>,
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
            Self::ContextCompactionStarted { .. } => "ContextCompactionStarted",
            Self::ContextCompactionCompleted { .. } => "ContextCompactionCompleted",
            Self::ContextCompactionFailed { .. } => "ContextCompactionFailed",
            Self::CompactionSummary { .. } => "CompactionSummary",
            Self::ModelProfileSwitched { .. } => "ModelProfileSwitched",
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
            Self::TaskCancelled { .. } => "TaskCancelled",
            Self::SessionCancelled { .. } => "SessionCancelled",
            Self::SkillDiscovered { .. } => "SkillDiscovered",
            Self::SkillValidationFailed { .. } => "SkillValidationFailed",
            Self::SkillActivated { .. } => "SkillActivated",
            Self::SkillDeactivated { .. } => "SkillDeactivated",
            Self::SkillSuggested { .. } => "SkillSuggested",
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
            Self::CatalogSourceAdded { .. } => "CatalogSourceAdded",
            Self::CatalogSourceFailed { .. } => "CatalogSourceFailed",
            Self::CatalogSourceResultsArrived { .. } => "CatalogSourceResultsArrived",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[test]
fn catalog_source_added_event_round_trips() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::CatalogSourceAdded {
            source: "mcp-registry".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event_type"], "CatalogSourceAdded");
    assert_eq!(json["payload"]["type"], "CatalogSourceAdded");
    assert_eq!(json["payload"]["source"], "mcp-registry");

    let s = serde_json::to_string(&event.payload).unwrap();
    let back: EventPayload = serde_json::from_str(&s).unwrap();
    assert!(
        matches!(back, EventPayload::CatalogSourceAdded { ref source } if source == "mcp-registry")
    );
}

#[test]
fn catalog_source_failed_event_round_trips() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::CatalogSourceFailed {
            source: "mcp-registry".into(),
            error: "timeout".into(),
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event_type"], "CatalogSourceFailed");
    assert_eq!(json["payload"]["type"], "CatalogSourceFailed");
    assert_eq!(json["payload"]["error"], "timeout");

    let s = serde_json::to_string(&event.payload).unwrap();
    let back: EventPayload = serde_json::from_str(&s).unwrap();
    assert!(
        matches!(back, EventPayload::CatalogSourceFailed { ref source, ref error }
        if source == "mcp-registry" && error == "timeout")
    );
}

#[test]
fn compaction_reason_serializes_with_internal_tag() {
    let r = CompactionReason::UserRequested;
    let json = serde_json::to_value(r).unwrap();
    assert_eq!(json["type"], "UserRequested");

    let r = CompactionReason::Threshold { ratio: 0.87 };
    let json = serde_json::to_value(r).unwrap();
    assert_eq!(json["type"], "Threshold");
    assert!((json["ratio"].as_f64().unwrap() - 0.87).abs() < 1e-6);
}

#[test]
fn context_compaction_started_event_round_trips() {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextCompactionStarted {
            reason: CompactionReason::Threshold { ratio: 0.9 },
            before_tokens: 180_000,
            candidate_event_count: 42,
        },
    );
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event_type"], "ContextCompactionStarted");
    assert_eq!(json["payload"]["type"], "ContextCompactionStarted");
    assert_eq!(json["payload"]["before_tokens"], 180_000);
    assert_eq!(json["payload"]["candidate_event_count"], 42);
    assert_eq!(json["payload"]["reason"]["type"], "Threshold");

    let s = serde_json::to_string(&event.payload).unwrap();
    let back: EventPayload = serde_json::from_str(&s).unwrap();
    assert!(matches!(
        back,
        EventPayload::ContextCompactionStarted { .. }
    ));
}

#[test]
fn context_compaction_completed_and_failed_round_trip() {
    let completed = EventPayload::ContextCompactionCompleted {
        summary_id: "sum_1".into(),
        after_tokens: 30_000,
        fallback_used: false,
    };
    let json = serde_json::to_value(&completed).unwrap();
    assert_eq!(json["type"], "ContextCompactionCompleted");
    assert_eq!(json["fallback_used"], false);
    let _back: EventPayload = serde_json::from_value(json).unwrap();

    let failed = EventPayload::ContextCompactionFailed {
        error: "model timeout".into(),
        fallback_used: true,
    };
    let json = serde_json::to_value(&failed).unwrap();
    assert_eq!(json["type"], "ContextCompactionFailed");
    assert_eq!(json["fallback_used"], true);
    let _back: EventPayload = serde_json::from_value(json).unwrap();
}

#[test]
fn compaction_summary_event_round_trips_with_timestamp_range() {
    use chrono::TimeZone;
    let from = chrono::Utc.with_ymd_and_hms(2026, 5, 8, 9, 0, 0).unwrap();
    let to = chrono::Utc.with_ymd_and_hms(2026, 5, 8, 10, 0, 0).unwrap();
    let payload = EventPayload::CompactionSummary {
        summary_id: "sum_1".into(),
        content: "## User goal\n...".into(),
        replaces_event_range: (from, to),
        reason: CompactionReason::UserRequested,
        before_tokens: 180_000,
        after_tokens: 4_000,
        summarised_by_profile: "fast".into(),
    };
    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["type"], "CompactionSummary");
    assert_eq!(json["summarised_by_profile"], "fast");
    let back: EventPayload = serde_json::from_value(json).unwrap();
    if let EventPayload::CompactionSummary {
        replaces_event_range,
        ..
    } = back
    {
        assert_eq!(replaces_event_range.0, from);
        assert_eq!(replaces_event_range.1, to);
    } else {
        panic!("wrong variant");
    }
}

#[test]
fn event_type_method_covers_new_compaction_variants() {
    let started = EventPayload::ContextCompactionStarted {
        reason: CompactionReason::UserRequested,
        before_tokens: 0,
        candidate_event_count: 0,
    };
    assert_eq!(started.event_type(), "ContextCompactionStarted");

    let completed = EventPayload::ContextCompactionCompleted {
        summary_id: "x".into(),
        after_tokens: 0,
        fallback_used: false,
    };
    assert_eq!(completed.event_type(), "ContextCompactionCompleted");

    let failed = EventPayload::ContextCompactionFailed {
        error: "x".into(),
        fallback_used: false,
    };
    assert_eq!(failed.event_type(), "ContextCompactionFailed");

    let summary = EventPayload::CompactionSummary {
        summary_id: "x".into(),
        content: "x".into(),
        replaces_event_range: (chrono::Utc::now(), chrono::Utc::now()),
        reason: CompactionReason::UserRequested,
        before_tokens: 0,
        after_tokens: 0,
        summarised_by_profile: "fast".into(),
    };
    assert_eq!(summary.event_type(), "CompactionSummary");
}

#[test]
fn context_assembled_payload_carries_usage_struct() {
    use crate::context_types::{ContextSource, ContextUsage};

    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextAssembled {
            usage: ContextUsage {
                total_tokens: 12_345,
                budget_tokens: 188_000,
                context_window: 200_000,
                output_reservation: 12_000,
                by_source: vec![
                    (ContextSource::System, 800),
                    (ContextSource::ToolDefinitions, 11_545),
                ],
                estimator: "cl100k_base".into(),
                corrected_by_real_usage: false,
            },
        },
    );

    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event_type"], "ContextAssembled");
    assert_eq!(json["payload"]["usage"]["total_tokens"], 12_345);
    assert_eq!(json["payload"]["usage"]["context_window"], 200_000);
    assert_eq!(json["payload"]["usage"]["estimator"], "cl100k_base");
    assert_eq!(json["payload"]["usage"]["by_source"][0][0], "system");
}

#[test]
fn model_profile_switched_event_round_trips() {
    use chrono::TimeZone;
    let effective_at = chrono::Utc.with_ymd_and_hms(2026, 5, 9, 10, 0, 0).unwrap();
    let payload = EventPayload::ModelProfileSwitched {
        from_profile: "fast".into(),
        to_profile: "claude-opus".into(),
        reasoning_effort: Some("high".into()),
        effective_at,
        context_window: 200_000,
        output_limit: 16_384,
        limit_source: "builtin_registry".into(),
    };

    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["type"], "ModelProfileSwitched");
    assert_eq!(json["from_profile"], "fast");
    assert_eq!(json["to_profile"], "claude-opus");
    assert_eq!(json["reasoning_effort"], "high");
    assert_eq!(json["context_window"], 200_000);
    assert_eq!(json["output_limit"], 16_384);
    assert_eq!(json["limit_source"], "builtin_registry");
    assert_eq!(json["effective_at"], "2026-05-09T10:00:00Z");

    let back: EventPayload = serde_json::from_value(json).unwrap();
    match back {
        EventPayload::ModelProfileSwitched {
            from_profile,
            to_profile,
            reasoning_effort,
            effective_at: at,
            context_window,
            output_limit,
            limit_source,
        } => {
            assert_eq!(from_profile, "fast");
            assert_eq!(to_profile, "claude-opus");
            assert_eq!(reasoning_effort.as_deref(), Some("high"));
            assert_eq!(at, effective_at);
            assert_eq!(context_window, 200_000);
            assert_eq!(output_limit, 16_384);
            assert_eq!(limit_source, "builtin_registry");
        }
        other => panic!("wrong variant: {other:?}"),
    }
}

#[test]
fn event_type_method_covers_model_profile_switched() {
    let p = EventPayload::ModelProfileSwitched {
        from_profile: "a".into(),
        to_profile: "b".into(),
        reasoning_effort: None,
        effective_at: chrono::Utc::now(),
        context_window: 0,
        output_limit: 0,
        limit_source: "fallback".into(),
    };
    assert_eq!(p.event_type(), "ModelProfileSwitched");
}

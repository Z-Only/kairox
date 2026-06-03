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

/// Why a turn-end auto-compaction trigger did NOT enqueue a compaction.
/// `BelowThreshold` is intentionally not modeled — it is the steady state
/// and would flood the event log.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type")]
pub enum CompactionSkipReason {
    AlreadyCompacting,
    ThresholdDisabled,
}

/// Why a background monitor process was stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type")]
pub enum MonitorStopReason {
    ExitCode { code: i32 },
    Timeout,
    UserStopped,
    SessionEnded,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display_content: Option<String>,
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
    /// Turn-end auto-compaction was suppressed. Emitted only for reasons
    /// that callers/UIs may want to surface; below-threshold is silent.
    ContextCompactionSkipped {
        reason: CompactionSkipReason,
        ratio: f32,
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
    MonitorStarted {
        monitor_id: String,
        description: String,
        command: String,
        persistent: bool,
        #[cfg_attr(feature = "specta", specta(type = u32))]
        timeout_ms: u64,
    },
    MonitorEvent {
        monitor_id: String,
        line: String,
    },
    MonitorStopped {
        monitor_id: String,
        reason: MonitorStopReason,
    },
    MonitorFailed {
        monitor_id: String,
        error: String,
    },
    LspServerStarting {
        server_id: String,
        languages: Vec<String>,
    },
    LspServerReady {
        server_id: String,
        languages: Vec<String>,
    },
    LspServerStopped {
        server_id: String,
    },
    LspServerFailed {
        server_id: String,
        error: String,
    },
    DapSessionStarted {
        server_id: String,
        program: String,
    },
    DapSessionStopped {
        server_id: String,
    },
    DapBreakpointHit {
        server_id: String,
        file: String,
        line: u32,
    },
    TrajectoryStarted {
        trajectory_id: String,
        task_id: String,
    },
    TrajectoryStepRecorded {
        trajectory_id: String,
        step_index: u32,
        action: String,
        observation_preview: String,
        screenshot_id: Option<String>,
        #[cfg_attr(feature = "specta", specta(type = u32))]
        duration_ms: u64,
    },
    TrajectoryCompleted {
        trajectory_id: String,
        step_count: u32,
        outcome: crate::trajectory::TrajectoryOutcome,
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
            Self::ContextCompactionSkipped { .. } => "ContextCompactionSkipped",
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
            Self::MonitorStarted { .. } => "MonitorStarted",
            Self::MonitorEvent { .. } => "MonitorEvent",
            Self::MonitorStopped { .. } => "MonitorStopped",
            Self::MonitorFailed { .. } => "MonitorFailed",
            Self::LspServerStarting { .. } => "LspServerStarting",
            Self::LspServerReady { .. } => "LspServerReady",
            Self::LspServerStopped { .. } => "LspServerStopped",
            Self::LspServerFailed { .. } => "LspServerFailed",
            Self::DapSessionStarted { .. } => "DapSessionStarted",
            Self::DapSessionStopped { .. } => "DapSessionStopped",
            Self::DapBreakpointHit { .. } => "DapBreakpointHit",
            Self::TrajectoryStarted { .. } => "TrajectoryStarted",
            Self::TrajectoryStepRecorded { .. } => "TrajectoryStepRecorded",
            Self::TrajectoryCompleted { .. } => "TrajectoryCompleted",
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
#[path = "events_tests.rs"]
mod tests;

//! Data types for the unified chat stream.
//!
//! Extracted from [`super::stream`] to keep the reducer logic and its
//! type definitions in separate, focused modules.

use agent_core::events::{CompactionSkipReason, MonitorStopReason};
use agent_core::projection::ProjectedRole;
use agent_core::TaskConfirmationOption;

/// Role of a [`ChatStreamItem::Message`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
}

impl From<ProjectedRole> for MessageRole {
    fn from(role: ProjectedRole) -> Self {
        match role {
            ProjectedRole::User => MessageRole::User,
            ProjectedRole::Assistant => MessageRole::Assistant,
        }
    }
}

/// Lifecycle status for a tool invocation surfaced in the chat stream.
///
/// `Requested` corresponds to [`EventPayload::ModelToolCallRequested`]
/// (the model has emitted a tool call but the runtime has not yet
/// started the invocation), `Running` to
/// [`EventPayload::ToolInvocationStarted`], `Completed` to
/// [`EventPayload::ToolInvocationCompleted`], and `Failed` to
/// [`EventPayload::ToolInvocationFailed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    Requested,
    Running,
    Completed,
    Failed,
}

/// Distinguishes tool-permission prompts from memory-write prompts so the
/// renderer can pick the right copy and affordances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionKind {
    Tool,
    Memory,
}

/// Resolution status for a [`ChatStreamItem::Permission`].
///
/// The reducer keeps `Accepted` / `Denied` items in the stream; the
/// renderer is responsible for filtering them out (matching the GUI's
/// behaviour where resolved permissions disappear from the inline feed
/// but stay visible in the trace timeline).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionStatus {
    Pending,
    Accepted,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskConfirmationStatus {
    Pending,
    Resolved,
}

/// Lifecycle status for a [`ChatStreamItem::Compaction`].
///
/// Mirrors [`agent_core::projection::CompactionStatus`] but specialised
/// to per-item lifecycle (one stream item per compaction run, not a
/// snapshot of the session's current state).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionItemStatus {
    Running,
    Completed,
    Failed,
}

/// Lifecycle status for a [`ChatStreamItem::Monitor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorItemStatus {
    Running,
    Stopped(MonitorStopReason),
    Failed,
}

/// One renderable row in the unified chat stream.
///
/// Each variant carries enough state for the follow-up renderer to draw
/// the row without re-walking the event log. `timestamp_ms` is the
/// event-time of the FIRST event that materialised the item (e.g. the
/// `PermissionRequested` event for a `Permission` item, not the later
/// `PermissionGranted`) so chronological ordering stays stable across
/// the item's full lifecycle.
// `Eq` is intentionally omitted: the `CompactionSkipped { ratio: f32 }`
// payload mirrors `EventPayload::ContextCompactionSkipped`, and `f32`
// does not implement `Eq` (NaN ≠ NaN). `PartialEq` is sufficient for
// every existing assertion in the test suites.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatStreamItem {
    Message {
        id: String,
        role: MessageRole,
        content: String,
        timestamp_ms: i64,
    },
    ToolCall {
        id: String,
        tool_id: String,
        /// Best-effort JSON serialisation of the tool arguments. The
        /// current `ModelToolCallRequested` event does not yet carry the
        /// arguments payload, so this is `""` for now and will be filled
        /// in by a later event-schema bump tracked in the campaign
        /// brief.
        args_json: String,
        status: ToolCallStatus,
        /// Populated by `ToolInvocationCompleted.output_preview` or
        /// `ToolInvocationFailed.error`.
        output_preview: Option<String>,
        duration_ms: Option<u64>,
        timestamp_ms: i64,
    },
    Permission {
        id: String,
        kind: PermissionKind,
        prompt: String,
        status: PermissionStatus,
        timestamp_ms: i64,
    },
    TaskConfirmation {
        id: String,
        prompt: String,
        options: Vec<TaskConfirmationOption>,
        allow_multiple: bool,
        allow_custom: bool,
        status: TaskConfirmationStatus,
        selected_option_ids: Vec<String>,
        custom_response: Option<String>,
        timestamp_ms: i64,
    },
    Compaction {
        id: String,
        status: CompactionItemStatus,
        progress_pct: Option<u8>,
        summary: Option<String>,
        /// Token count before the compaction ran, lifted from
        /// [`EventPayload::ContextCompactionStarted::before_tokens`].
        /// `None` if the started event was not observed.
        before_tokens: Option<u64>,
        /// Token count after the compaction completed, lifted from
        /// [`EventPayload::ContextCompactionCompleted::after_tokens`].
        /// `None` for in-flight (`Running`) or `Failed` compactions.
        after_tokens: Option<u64>,
        timestamp_ms: i64,
    },
    /// A turn-end auto-compaction trigger that did NOT run, surfaced so
    /// the user can see *why* nothing happened (v0.31.0 "UIs can explain
    /// inaction" promise). Has no lifecycle — fires once and renders
    /// once. The `ratio` is the assembled-context ratio at the moment
    /// the skip was decided; renderers may omit it when it carries no
    /// useful signal (e.g. `ThresholdDisabled`).
    CompactionSkipped {
        id: String,
        reason: CompactionSkipReason,
        ratio: f32,
        timestamp_ms: i64,
    },
    Monitor {
        id: String,
        monitor_id: String,
        description: String,
        status: MonitorItemStatus,
        last_line: Option<String>,
        timestamp_ms: i64,
    },
}

impl ChatStreamItem {
    /// The event-time anchor for chronological ordering. Always returns
    /// the timestamp of the FIRST event that materialised the item so
    /// lifecycle updates (e.g. a permission being granted later) don't
    /// reorder the stream.
    pub fn timestamp_ms(&self) -> i64 {
        match self {
            Self::Message { timestamp_ms, .. } => *timestamp_ms,
            Self::ToolCall { timestamp_ms, .. } => *timestamp_ms,
            Self::Permission { timestamp_ms, .. } => *timestamp_ms,
            Self::TaskConfirmation { timestamp_ms, .. } => *timestamp_ms,
            Self::Compaction { timestamp_ms, .. } => *timestamp_ms,
            Self::CompactionSkipped { timestamp_ms, .. } => *timestamp_ms,
            Self::Monitor { timestamp_ms, .. } => *timestamp_ms,
        }
    }

    /// Stable identifier — useful as a UI list key. Derived from the
    /// underlying event ids (message id, tool-call id, request id,
    /// synthetic compaction id) so it survives re-renders.
    pub fn id(&self) -> &str {
        match self {
            Self::Message { id, .. } => id,
            Self::ToolCall { id, .. } => id,
            Self::Permission { id, .. } => id,
            Self::TaskConfirmation { id, .. } => id,
            Self::Compaction { id, .. } => id,
            Self::CompactionSkipped { id, .. } => id,
            Self::Monitor { id, .. } => id,
        }
    }
}

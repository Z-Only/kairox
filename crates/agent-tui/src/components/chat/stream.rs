//! `ChatStreamItem` discriminated union and the `fold_stream` reducer.
//!
//! Mirrors the GUI `useChatStream` composable
//! (`apps/agent-gui/src/composables/useChatStream.ts`) on the Rust side:
//! folds a session's domain events plus its
//! [`SessionProjection`](agent_core::projection::SessionProjection) into
//! a single chronologically ordered list of items that the renderer
//! (R3-2 follow-up PR) will draw inline inside `ChatPanel` —
//! messages, tool calls, permission prompts, and context-compaction
//! progress all in one feed instead of split across the chat pane and a
//! modal.
//!
//! This module is foundation only. It does NOT touch
//! [`crate::components::chat::ChatPanel`] rendering, the existing
//! permission modal, or any GUI surface. The renderer integration —
//! plus removing the standalone permission modal — ships in a
//! dependent follow-up PR.
//!
//! ## Filter rule
//!
//! The GUI drops `accepted` / `denied` permission entries from the
//! inline chat stream once they resolve (one-shot UI). The reducer here
//! keeps them — surfacing `status` so the renderer can decide whether
//! to hide terminal permissions or fade them out. This matches the
//! `traceEntries`-as-source contract in the GUI, where the trace store
//! retains resolved permissions and the composable filters them.

use std::collections::HashMap;

use agent_core::events::{DomainEvent, EventPayload};
use agent_core::projection::{ProjectedRole, SessionProjection};

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

/// One renderable row in the unified chat stream.
///
/// Each variant carries enough state for the follow-up renderer to draw
/// the row without re-walking the event log. `timestamp_ms` is the
/// event-time of the FIRST event that materialised the item (e.g. the
/// `PermissionRequested` event for a `Permission` item, not the later
/// `PermissionGranted`) so chronological ordering stays stable across
/// the item's full lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    Compaction {
        id: String,
        status: CompactionItemStatus,
        progress_pct: Option<u8>,
        summary: Option<String>,
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
            Self::Compaction { timestamp_ms, .. } => *timestamp_ms,
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
            Self::Compaction { id, .. } => id,
        }
    }
}

/// Fold a session's domain events plus its projection into a single
/// chronologically ordered list of [`ChatStreamItem`]s.
///
/// `projection` is currently unused by the reducer — items are derived
/// entirely from the event log so each item can carry an
/// event-anchored `timestamp_ms`. The parameter is kept on the
/// signature both to mirror the GUI composable's input shape and so
/// the follow-up renderer can pass the same projection it already
/// holds without an extra plumbing change.
///
/// The reducer never mutates either input.
pub fn fold_stream(_projection: &SessionProjection, events: &[DomainEvent]) -> Vec<ChatStreamItem> {
    let mut items: Vec<ChatStreamItem> = Vec::new();
    // tool_call_id (or invocation_id) -> index into `items`.
    let mut tool_call_index: HashMap<String, usize> = HashMap::new();
    // request_id (tool permission) or memory_id (memory permission) -> index.
    let mut permission_index: HashMap<String, usize> = HashMap::new();
    // summary_id -> index for the matching Compaction item (set on Completed,
    // used later by CompactionSummary to fill in `summary`).
    let mut compaction_index: HashMap<String, usize> = HashMap::new();
    // Stack of indices of in-flight (`Running`) compaction items, so
    // `Completed` / `Failed` can resolve the most recent unresolved run.
    let mut pending_compaction: Vec<usize> = Vec::new();
    let mut compaction_counter: usize = 0;

    for event in events {
        let timestamp_ms = event.timestamp.timestamp_millis();
        match &event.payload {
            EventPayload::UserMessageAdded {
                message_id,
                content,
            } => {
                items.push(ChatStreamItem::Message {
                    id: message_id.clone(),
                    role: MessageRole::User,
                    content: content.clone(),
                    timestamp_ms,
                });
            }
            EventPayload::AssistantMessageCompleted {
                message_id,
                content,
            } => {
                items.push(ChatStreamItem::Message {
                    id: message_id.clone(),
                    role: MessageRole::Assistant,
                    content: content.clone(),
                    timestamp_ms,
                });
            }
            EventPayload::ModelToolCallRequested {
                tool_call_id,
                tool_id,
            } => {
                let idx = items.len();
                items.push(ChatStreamItem::ToolCall {
                    id: tool_call_id.clone(),
                    tool_id: tool_id.clone(),
                    args_json: String::new(),
                    status: ToolCallStatus::Requested,
                    output_preview: None,
                    duration_ms: None,
                    timestamp_ms,
                });
                tool_call_index.insert(tool_call_id.clone(), idx);
            }
            EventPayload::ToolInvocationStarted {
                invocation_id,
                tool_id,
            } => {
                if let Some(idx) = tool_call_index
                    .get(invocation_id)
                    .copied()
                    .or_else(|| find_latest_unresolved_tool_call(&items, tool_id))
                {
                    if let ChatStreamItem::ToolCall { status, .. } = &mut items[idx] {
                        *status = ToolCallStatus::Running;
                    }
                    // Alias the invocation_id onto the same item so a
                    // later `ToolInvocationCompleted/Failed` with this
                    // invocation_id resolves the right row even if the
                    // tool_call_id and invocation_id differ.
                    tool_call_index.insert(invocation_id.clone(), idx);
                } else {
                    let idx = items.len();
                    items.push(ChatStreamItem::ToolCall {
                        id: invocation_id.clone(),
                        tool_id: tool_id.clone(),
                        args_json: String::new(),
                        status: ToolCallStatus::Running,
                        output_preview: None,
                        duration_ms: None,
                        timestamp_ms,
                    });
                    tool_call_index.insert(invocation_id.clone(), idx);
                }
            }
            EventPayload::ToolInvocationCompleted {
                invocation_id,
                tool_id,
                output_preview,
                duration_ms,
                ..
            } => {
                if let Some(idx) = tool_call_index
                    .get(invocation_id)
                    .copied()
                    .or_else(|| find_latest_unresolved_tool_call(&items, tool_id))
                {
                    if let ChatStreamItem::ToolCall {
                        status,
                        output_preview: out,
                        duration_ms: dur,
                        ..
                    } = &mut items[idx]
                    {
                        *status = ToolCallStatus::Completed;
                        *out = Some(output_preview.clone());
                        *dur = Some(*duration_ms);
                    }
                } else {
                    items.push(ChatStreamItem::ToolCall {
                        id: invocation_id.clone(),
                        tool_id: tool_id.clone(),
                        args_json: String::new(),
                        status: ToolCallStatus::Completed,
                        output_preview: Some(output_preview.clone()),
                        duration_ms: Some(*duration_ms),
                        timestamp_ms,
                    });
                }
            }
            EventPayload::ToolInvocationFailed {
                invocation_id,
                tool_id,
                error,
            } => {
                if let Some(idx) = tool_call_index
                    .get(invocation_id)
                    .copied()
                    .or_else(|| find_latest_unresolved_tool_call(&items, tool_id))
                {
                    if let ChatStreamItem::ToolCall {
                        status,
                        output_preview: out,
                        ..
                    } = &mut items[idx]
                    {
                        *status = ToolCallStatus::Failed;
                        *out = Some(error.clone());
                    }
                } else {
                    items.push(ChatStreamItem::ToolCall {
                        id: invocation_id.clone(),
                        tool_id: tool_id.clone(),
                        args_json: String::new(),
                        status: ToolCallStatus::Failed,
                        output_preview: Some(error.clone()),
                        duration_ms: None,
                        timestamp_ms,
                    });
                }
            }
            EventPayload::PermissionRequested {
                request_id,
                tool_id: _,
                preview,
            } => {
                let idx = items.len();
                items.push(ChatStreamItem::Permission {
                    id: request_id.clone(),
                    kind: PermissionKind::Tool,
                    prompt: preview.clone(),
                    status: PermissionStatus::Pending,
                    timestamp_ms,
                });
                permission_index.insert(request_id.clone(), idx);
            }
            EventPayload::PermissionGranted { request_id } => {
                if let Some(idx) = permission_index.get(request_id).copied() {
                    if let ChatStreamItem::Permission { status, .. } = &mut items[idx] {
                        *status = PermissionStatus::Accepted;
                    }
                }
            }
            EventPayload::PermissionDenied { request_id, .. } => {
                if let Some(idx) = permission_index.get(request_id).copied() {
                    if let ChatStreamItem::Permission { status, .. } = &mut items[idx] {
                        *status = PermissionStatus::Denied;
                    }
                }
            }
            EventPayload::MemoryProposed {
                memory_id,
                scope,
                key,
                content,
            } => {
                let prompt = match key {
                    Some(k) => format!("memory[{scope}:{k}]: {content}"),
                    None => format!("memory[{scope}]: {content}"),
                };
                let idx = items.len();
                items.push(ChatStreamItem::Permission {
                    id: memory_id.clone(),
                    kind: PermissionKind::Memory,
                    prompt,
                    status: PermissionStatus::Pending,
                    timestamp_ms,
                });
                permission_index.insert(memory_id.clone(), idx);
            }
            EventPayload::MemoryAccepted { memory_id, .. } => {
                if let Some(idx) = permission_index.get(memory_id).copied() {
                    if let ChatStreamItem::Permission { status, .. } = &mut items[idx] {
                        *status = PermissionStatus::Accepted;
                    }
                }
            }
            EventPayload::MemoryRejected { memory_id, .. } => {
                if let Some(idx) = permission_index.get(memory_id).copied() {
                    if let ChatStreamItem::Permission { status, .. } = &mut items[idx] {
                        *status = PermissionStatus::Denied;
                    }
                }
            }
            EventPayload::ContextCompactionStarted { .. } => {
                compaction_counter += 1;
                let idx = items.len();
                items.push(ChatStreamItem::Compaction {
                    id: format!("compaction-{compaction_counter}"),
                    status: CompactionItemStatus::Running,
                    progress_pct: None,
                    summary: None,
                    timestamp_ms,
                });
                pending_compaction.push(idx);
            }
            EventPayload::ContextCompactionCompleted { summary_id, .. } => {
                if let Some(idx) = pending_compaction.pop() {
                    if let ChatStreamItem::Compaction { status, .. } = &mut items[idx] {
                        *status = CompactionItemStatus::Completed;
                    }
                    compaction_index.insert(summary_id.clone(), idx);
                }
            }
            EventPayload::ContextCompactionFailed { error, .. } => {
                if let Some(idx) = pending_compaction.pop() {
                    if let ChatStreamItem::Compaction {
                        status, summary, ..
                    } = &mut items[idx]
                    {
                        *status = CompactionItemStatus::Failed;
                        *summary = Some(error.clone());
                    }
                }
            }
            EventPayload::CompactionSummary {
                summary_id,
                content,
                ..
            } => {
                if let Some(idx) = compaction_index.get(summary_id).copied() {
                    if let ChatStreamItem::Compaction { summary, .. } = &mut items[idx] {
                        *summary = Some(content.clone());
                    }
                }
            }
            // All other events are surfaced elsewhere (trace panel, task
            // graph, MCP overlay, status bar, etc.) and have no inline
            // chat-stream representation.
            _ => {}
        }
    }

    items
}

/// Walk `items` from newest to oldest and return the index of the most
/// recent [`ChatStreamItem::ToolCall`] for `tool_id` whose status is
/// not yet terminal (`Completed` / `Failed`). Used as a fallback when
/// an invocation_id does not match any tracked tool_call_id.
fn find_latest_unresolved_tool_call(items: &[ChatStreamItem], tool_id: &str) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .rev()
        .find_map(|(idx, item)| match item {
            ChatStreamItem::ToolCall {
                tool_id: tid,
                status,
                ..
            } if tid == tool_id
                && !matches!(status, ToolCallStatus::Completed | ToolCallStatus::Failed) =>
            {
                Some(idx)
            }
            _ => None,
        })
}

//! Inline rendering for the unified ChatPanel feed.
//!
//! Renders the chat scrollback as a single unified stream by combining:
//!
//! - **Messages** — read from
//!   [`SessionProjection::messages`](agent_core::projection::SessionProjection),
//!   matching the GUI [`useChatStream`](../../../../../../apps/agent-gui/src/composables/useChatStream.ts)
//!   composable which also treats `projection.messages` as the source
//!   of truth for the message stream (not the raw event log).
//! - **Permissions / Tool calls / Compaction** — folded from the
//!   session's domain event log via
//!   [`fold_stream`](super::stream::fold_stream), which carries the
//!   chronological position needed to interleave them after messages.
//!
//! The order matches GUI parity: messages in projection order first,
//! then non-message stream items in chronological (`timestamp_ms`)
//! order. Mirrors the visual intent of `ChatMessageItem.vue`,
//! `ChatPermissionItem.vue`, `ChatToolCallItem.vue`, and
//! `ChatCompactionItem.vue` at the TUI's information density.
//!
//! ## Filter rules (parity with `useChatStream.ts`)
//!
//! - Resolved permissions (`Accepted` / `Denied`) are filtered out of
//!   the inline stream — they remain available in the trace timeline.
//! - Only the most recent compaction item renders when terminal
//!   (`Completed`/`Failed`); the in-flight (`Running`) banner always
//!   renders, even if older terminal items exist.
//!
//! ## Exit-code lookup
//!
//! The reducer in [`stream`](super::stream) intentionally does NOT
//! carry the tool-invocation exit code through the item; the renderer
//! does a single O(events) pre-pass to build a `HashMap<invocation_id,
//! exit_code>` so expanded tool calls can show `exit=N`. See the
//! follow-up note in the campaign brief for surfacing exit_code on the
//! stream item itself.

use std::collections::{HashMap, HashSet};

use crate::app_state::InputState;
use agent_core::events::{DomainEvent, EventPayload};
use agent_core::projection::SessionProjection;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use super::stream::{
    fold_stream, ChatStreamItem, CompactionItemStatus, MessageRole, PermissionStatus,
    TaskConfirmationStatus,
};
use super::stream_render_items::{
    append_compaction, append_compaction_skipped, append_message, append_monitor,
    append_permission, append_task_confirmation, append_tool_call,
};

/// Render the entire unified chat-stream feed into `area`.
///
/// `projection` provides the latest `token_stream` and `cancelled` flag
/// (which are NOT part of the discrete event log but still belong in
/// the chat scrollback). `events` is the chronological domain-event log
/// for the active session, folded through
/// [`fold_stream`](super::stream::fold_stream). `expanded_tool_calls`
/// is the per-session set of tool-call invocation ids the user has
/// opted to expand.
pub fn render_chat_stream(
    area: Rect,
    frame: &mut Frame,
    projection: &SessionProjection,
    events: &[DomainEvent],
    expanded_tool_calls: &HashSet<String>,
    input_state: &InputState,
) {
    let items = fold_stream(projection, events);
    let exit_codes = build_exit_code_map(events);
    let permission_tool_ids = build_permission_tool_id_map(events);
    let last_terminal_compaction = find_last_terminal_compaction(&items);

    let mut lines: Vec<Line> = Vec::new();

    // Messages: read from projection (GUI parity with useChatStream.ts).
    for msg in &projection.messages {
        let role = match msg.role {
            agent_core::projection::ProjectedRole::User => MessageRole::User,
            agent_core::projection::ProjectedRole::Assistant => MessageRole::Assistant,
        };
        append_message(&mut lines, role, &msg.content);
    }

    // Non-message items: fold_stream output in chronological order.
    let mut last_ts: i64 = i64::MIN;
    for (idx, item) in items.iter().enumerate() {
        debug_assert!(
            item.timestamp_ms() >= last_ts,
            "fold_stream broke chronological contract at id={}",
            item.id()
        );
        last_ts = item.timestamp_ms();
        match item {
            ChatStreamItem::Message { .. } => {}
            ChatStreamItem::Permission { status, .. } => {
                if matches!(status, PermissionStatus::Pending) {
                    let tool_id_lookup = permission_tool_ids.get(item.id()).map(String::as_str);
                    append_permission(&mut lines, item, tool_id_lookup);
                }
            }
            ChatStreamItem::TaskConfirmation { status, .. } => {
                if matches!(status, TaskConfirmationStatus::Pending) {
                    let selected =
                        task_confirmation_selection(input_state, item.id()).unwrap_or(&[]);
                    append_task_confirmation(&mut lines, item, selected);
                }
            }
            ChatStreamItem::ToolCall { .. } => {
                let expanded = expanded_tool_calls.contains(item.id());
                append_tool_call(&mut lines, item, expanded, &exit_codes);
            }
            ChatStreamItem::Compaction { status, .. } => match status {
                CompactionItemStatus::Running => append_compaction(&mut lines, item),
                CompactionItemStatus::Completed | CompactionItemStatus::Failed => {
                    if Some(idx) == last_terminal_compaction {
                        append_compaction(&mut lines, item);
                    }
                }
            },
            ChatStreamItem::CompactionSkipped { reason, ratio, .. } => {
                append_compaction_skipped(&mut lines, *reason, *ratio);
            }
            ChatStreamItem::Monitor { .. } => {
                append_monitor(&mut lines, item);
            }
        }
    }

    if !projection.token_stream.is_empty() {
        let stream_text = format!("{}▌", projection.token_stream);
        lines.push(Line::from(vec![
            Span::styled("Agent: ", Style::default().fg(Color::Green)),
            Span::raw(stream_text),
        ]));
    }

    if projection.cancelled {
        lines.push(Line::from(Span::styled(
            "[cancelled]",
            Style::default().fg(Color::Yellow),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn task_confirmation_selection<'a>(
    input_state: &'a InputState,
    request_id: &str,
) -> Option<&'a [String]> {
    match input_state {
        InputState::TaskConfirmationWait {
            request_id: active_id,
            selected_option_ids,
            ..
        } if active_id == request_id => Some(selected_option_ids.as_slice()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// event pre-pass helpers
// ---------------------------------------------------------------------------

fn build_exit_code_map(events: &[DomainEvent]) -> HashMap<String, i32> {
    let mut map = HashMap::new();
    for event in events {
        if let EventPayload::ToolInvocationCompleted {
            invocation_id,
            exit_code: Some(code),
            ..
        } = &event.payload
        {
            map.insert(invocation_id.clone(), *code);
        }
    }
    map
}

fn build_permission_tool_id_map(events: &[DomainEvent]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for event in events {
        if let EventPayload::PermissionRequested {
            request_id,
            tool_id,
            ..
        } = &event.payload
        {
            map.insert(request_id.clone(), tool_id.clone());
        }
    }
    map
}

fn find_last_terminal_compaction(items: &[ChatStreamItem]) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .rev()
        .find_map(|(idx, item)| match item {
            ChatStreamItem::Compaction {
                status: CompactionItemStatus::Completed | CompactionItemStatus::Failed,
                ..
            } => Some(idx),
            _ => None,
        })
}

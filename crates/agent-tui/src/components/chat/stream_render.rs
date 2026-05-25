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

use agent_core::events::{DomainEvent, EventPayload};
use agent_core::projection::SessionProjection;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use super::stream::{
    fold_stream, ChatStreamItem, CompactionItemStatus, MessageRole, PermissionKind,
    PermissionStatus, ToolCallStatus,
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
    // We still iterate Message items to assert the reducer's
    // chronological contract via the public id/timestamp accessors;
    // they're skipped for rendering since projection.messages is the
    // source of truth above.
    let mut last_ts: i64 = i64::MIN;
    for (idx, item) in items.iter().enumerate() {
        debug_assert!(
            item.timestamp_ms() >= last_ts,
            "fold_stream broke chronological contract at id={}",
            item.id()
        );
        last_ts = item.timestamp_ms();
        match item {
            ChatStreamItem::Message { .. } => {
                // Already rendered from projection.messages above.
            }
            ChatStreamItem::Permission { status, .. } => {
                if matches!(status, PermissionStatus::Pending) {
                    let tool_id_lookup = permission_tool_ids.get(item.id()).map(String::as_str);
                    append_permission(&mut lines, item, tool_id_lookup);
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

// ---------------------------------------------------------------------------
// per-item helpers
// ---------------------------------------------------------------------------

fn append_message(lines: &mut Vec<Line>, role: MessageRole, content: &str) {
    let (label, color) = match role {
        MessageRole::User => ("You:", Color::Cyan),
        MessageRole::Assistant => ("Agent:", Color::Green),
    };
    let content_lines: Vec<&str> = content.split('\n').collect();
    for (i, line) in content_lines.iter().enumerate() {
        if i == 0 {
            lines.push(Line::from(vec![
                Span::styled(format!("{label} "), Style::default().fg(color)),
                Span::raw(line.to_string()),
            ]));
        } else {
            lines.push(Line::raw(line.to_string()));
        }
    }
}

fn append_permission(lines: &mut Vec<Line>, item: &ChatStreamItem, tool_id_lookup: Option<&str>) {
    let (kind, prompt) = match item {
        ChatStreamItem::Permission { kind, prompt, .. } => (*kind, prompt.as_str()),
        _ => return,
    };

    let border_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let header = match kind {
        PermissionKind::Tool => "╭─ Permission required ",
        PermissionKind::Memory => "╭─ Memory write required ",
    };
    lines.push(Line::from(Span::styled(
        header.to_string() + &"─".repeat(40),
        border_style,
    )));
    // Memory prompts encode their target in the prompt; tool prompts pair
    // with an event-derived tool_id lookup so the renderer can show the
    // qualified id even though the reducer's Permission item drops it.
    let tool_id_value = match kind {
        PermissionKind::Tool => tool_id_lookup.unwrap_or("(unknown)"),
        PermissionKind::Memory => "memory",
    };
    lines.push(Line::from(vec![
        Span::styled("│ ", border_style),
        Span::styled(
            "tool: ",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(tool_id_value.to_string(), Style::default().fg(Color::White)),
    ]));
    for (i, preview_line) in prompt.split('\n').enumerate() {
        let prefix = if i == 0 { "preview: " } else { "         " };
        lines.push(Line::from(vec![
            Span::styled("│ ", border_style),
            Span::styled(
                prefix,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(preview_line.to_string(), Style::default().fg(Color::White)),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled("│ ", border_style),
        Span::styled(
            "press ",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "Y/N/D",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " to allow / deny / deny-all",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "╰".to_string() + &"─".repeat(62),
        border_style,
    )));
}

fn append_tool_call(
    lines: &mut Vec<Line>,
    item: &ChatStreamItem,
    expanded: bool,
    exit_codes: &HashMap<String, i32>,
) {
    let (id, tool_id, status, output_preview, duration_ms) = match item {
        ChatStreamItem::ToolCall {
            id,
            tool_id,
            status,
            output_preview,
            duration_ms,
            ..
        } => (
            id,
            tool_id,
            *status,
            output_preview.as_deref(),
            *duration_ms,
        ),
        _ => return,
    };

    let (status_label, status_color) = match status {
        ToolCallStatus::Requested => ("requested", Color::Yellow),
        ToolCallStatus::Running => ("running", Color::Yellow),
        ToolCallStatus::Completed => ("completed", Color::Green),
        ToolCallStatus::Failed => ("failed", Color::Red),
    };

    let marker = if expanded { "▾" } else { "▸" };
    let mut header_spans = vec![
        Span::styled(
            format!("{marker} "),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            tool_id.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            status_label.to_string(),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(dur) = duration_ms {
        header_spans.push(Span::raw("  "));
        header_spans.push(Span::styled(
            format_duration(dur),
            Style::default().fg(Color::DarkGray),
        ));
    }
    lines.push(Line::from(header_spans));

    if !expanded {
        if status == ToolCallStatus::Failed {
            if let Some(error) = output_preview {
                for line in error.split('\n').take(3) {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(line.to_string(), Style::default().fg(Color::Red)),
                    ]));
                }
            }
        }
        return;
    }

    // Expanded form: full output_preview + exit_code lookup.
    if let Some(output) = output_preview {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "output:",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        for line in output.split('\n') {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(line.to_string(), Style::default().fg(Color::White)),
            ]));
        }
    }
    if let Some(exit) = exit_codes.get(id) {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("exit={exit}"), Style::default().fg(Color::DarkGray)),
        ]));
    }
}

fn append_compaction(lines: &mut Vec<Line>, item: &ChatStreamItem) {
    let (status, summary, before_tokens, after_tokens) = match item {
        ChatStreamItem::Compaction {
            status,
            summary,
            before_tokens,
            after_tokens,
            ..
        } => (*status, summary.as_deref(), *before_tokens, *after_tokens),
        _ => return,
    };
    let style = Style::default().add_modifier(Modifier::BOLD);
    match status {
        CompactionItemStatus::Running => {
            lines.push(Line::from(Span::styled(
                "⟳ Compacting context...".to_string(),
                style.fg(Color::Yellow),
            )));
        }
        CompactionItemStatus::Completed => {
            let mut spans = vec![Span::styled(
                "✓ Compacted".to_string(),
                style.fg(Color::Green),
            )];
            if let (Some(before), Some(after)) = (before_tokens, after_tokens) {
                let pct = compaction_savings_pct(before, after);
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    format!("{before} → {after} tokens (-{pct}%)"),
                    Style::default().fg(Color::Green),
                ));
            }
            if let Some(summary) = summary {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    truncate_chars(summary, 80),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::from(spans));
        }
        CompactionItemStatus::Failed => {
            let mut spans = vec![Span::styled(
                "✗ Compaction failed".to_string(),
                style.fg(Color::Red),
            )];
            if let Some(error) = summary {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    truncate_chars(error, 80),
                    Style::default().fg(Color::Red),
                ));
            }
            lines.push(Line::from(spans));
        }
    }
}

/// Percent reduction from `before` to `after`, clamped to `0..=100`.
/// Returns `0` when `before == 0` (guarding the division) and uses
/// saturating subtraction so post-compaction inflation also yields
/// `0%` rather than a negative or wrapped value.
fn compaction_savings_pct(before: u64, after: u64) -> i32 {
    if before == 0 {
        return 0;
    }
    let saved = before.saturating_sub(after);
    let pct = (saved as f64 / before as f64 * 100.0).round() as i32;
    pct.clamp(0, 100)
}

// ---------------------------------------------------------------------------
// helpers
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

/// Build a map of permission request_id → tool_id from
/// `PermissionRequested` events. The reducer's `Permission` stream item
/// drops the originating `tool_id`, but the renderer needs it for the
/// permission row's header. See the campaign brief follow-up note.
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

fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1_000.0)
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_duration_renders_ms_under_one_second() {
        assert_eq!(format_duration(120), "120ms");
        assert_eq!(format_duration(999), "999ms");
    }

    #[test]
    fn format_duration_renders_seconds_at_or_above_one_second() {
        assert_eq!(format_duration(1_000), "1.0s");
        assert_eq!(format_duration(2_500), "2.5s");
    }

    #[test]
    fn truncate_chars_appends_ellipsis_when_long() {
        assert_eq!(truncate_chars("hello", 10), "hello");
        assert_eq!(truncate_chars("hello world", 5), "hello…");
    }

    #[test]
    fn compaction_savings_pct_rounds_and_handles_edges() {
        // Canonical case from the snapshot test: 25k → 12k = -52%.
        assert_eq!(compaction_savings_pct(25_000, 12_000), 52);
        // No reduction → 0%, not negative.
        assert_eq!(compaction_savings_pct(10_000, 10_000), 0);
        // Post-compaction inflation should not wrap; clamps to 0%.
        assert_eq!(compaction_savings_pct(10_000, 12_000), 0);
        // Div-by-zero guard.
        assert_eq!(compaction_savings_pct(0, 0), 0);
        assert_eq!(compaction_savings_pct(0, 5_000), 0);
        // Full reduction → 100%.
        assert_eq!(compaction_savings_pct(10_000, 0), 100);
    }
}

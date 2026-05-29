//! Per-item rendering helpers for the chat stream.
//!
//! Each function appends styled [`Line`]s for one stream item variant.
//! Extracted from [`super::stream_render`] to keep the orchestrator
//! focused on sequencing and filter rules while these helpers own the
//! visual representation of each item type.

use std::collections::HashMap;

use agent_core::events::{CompactionSkipReason, MonitorStopReason};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::stream::{
    ChatStreamItem, CompactionItemStatus, MessageRole, MonitorItemStatus, PermissionKind,
    ToolCallStatus,
};

pub fn append_message(lines: &mut Vec<Line>, role: MessageRole, content: &str) {
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

pub fn append_permission(
    lines: &mut Vec<Line>,
    item: &ChatStreamItem,
    tool_id_lookup: Option<&str>,
) {
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

pub fn append_tool_call(
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

pub fn append_compaction(lines: &mut Vec<Line>, item: &ChatStreamItem) {
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

pub fn append_compaction_skipped(lines: &mut Vec<Line>, reason: CompactionSkipReason, ratio: f32) {
    let style = Style::default().add_modifier(Modifier::BOLD);
    let reason_phrase = match reason {
        CompactionSkipReason::AlreadyCompacting => "another compaction in flight",
        CompactionSkipReason::ThresholdDisabled => "threshold disabled",
    };
    let mut spans = vec![Span::styled(
        format!("⊘ Compaction skipped: {reason_phrase}"),
        style.fg(Color::DarkGray),
    )];
    if !matches!(reason, CompactionSkipReason::ThresholdDisabled) {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("(ratio {ratio:.2})"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    lines.push(Line::from(spans));
}

pub fn append_monitor(lines: &mut Vec<Line>, item: &ChatStreamItem) {
    let (description, status, last_line) = match item {
        ChatStreamItem::Monitor {
            description,
            status,
            last_line,
            ..
        } => (description.as_str(), *status, last_line.as_deref()),
        _ => return,
    };

    let (icon, label, color) = match status {
        MonitorItemStatus::Running => ("⟳", "watching", Color::Yellow),
        MonitorItemStatus::Stopped(reason) => {
            let label = match reason {
                MonitorStopReason::ExitCode { code } => {
                    if code == 0 {
                        "done"
                    } else {
                        "exited"
                    }
                }
                MonitorStopReason::Timeout => "timed out",
                MonitorStopReason::UserStopped => "stopped",
                MonitorStopReason::SessionEnded => "ended",
            };
            let color = match reason {
                MonitorStopReason::ExitCode { code: 0 } => Color::Green,
                _ => Color::DarkGray,
            };
            ("■", label, color)
        }
        MonitorItemStatus::Failed => ("✗", "failed", Color::Red),
    };

    let style = Style::default().add_modifier(Modifier::BOLD);
    lines.push(Line::from(vec![
        Span::styled(format!("{icon} "), style.fg(color)),
        Span::styled(description.to_string(), style.fg(color)),
        Span::raw("  "),
        Span::styled(label.to_string(), Style::default().fg(color)),
    ]));

    if let Some(line) = last_line {
        let truncated = truncate_chars(line, 120);
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(truncated, Style::default().fg(Color::DarkGray)),
        ]));
    }
}

pub fn compaction_savings_pct(before: u64, after: u64) -> i32 {
    if before == 0 {
        return 0;
    }
    let saved = before.saturating_sub(after);
    let pct = (saved as f64 / before as f64 * 100.0).round() as i32;
    pct.clamp(0, 100)
}

pub fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1_000.0)
    }
}

pub fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

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
        assert_eq!(compaction_savings_pct(25_000, 12_000), 52);
        assert_eq!(compaction_savings_pct(10_000, 10_000), 0);
        assert_eq!(compaction_savings_pct(10_000, 12_000), 0);
        assert_eq!(compaction_savings_pct(0, 0), 0);
        assert_eq!(compaction_savings_pct(0, 5_000), 0);
        assert_eq!(compaction_savings_pct(10_000, 0), 100);
    }
}

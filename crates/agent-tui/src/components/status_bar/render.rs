//! Standalone render helpers for the single-line status bar.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::components::StatusInfo;

use super::context_line::render_context_line_string;

/// Render a single-line status bar into the given area.
///
/// Layout (left → right):
///
/// ```text
/// [ profile ] [ mode ] sessions: N  [MCP:N↑]  hint text  error!
/// ```
///
/// - **profile** — cyan background, bold
/// - **permission mode** — yellow background, bold
/// - **session count** — default style
/// - **MCP server count** — magenta, shown only if > 0
/// - **hint** — dim
/// - **error** (if present) — red foreground, bold
#[allow(dead_code)]
pub fn render_status_bar(area: Rect, frame: &mut Frame, info: &StatusInfo) {
    render_status_bar_with_notification(area, frame, info, None);
}

pub(super) fn render_status_bar_with_notification(
    area: Rect,
    frame: &mut Frame,
    info: &StatusInfo,
    notification: Option<&str>,
) {
    // P3: when we have observed at least one ContextAssembled event, switch
    // to the dedicated context-meter line. The legacy renderer below remains
    // the fallback for the cold-start case (no usage yet).
    if info.context_usage.is_some() {
        let mut line_text = render_context_line_string(info, area.width);
        if let Some(notification) = notification.filter(|value| !value.is_empty()) {
            line_text.push_str("  status: ");
            line_text.push_str(notification);
        }
        frame.render_widget(Paragraph::new(Line::from(Span::raw(line_text))), area);
        return;
    }

    let mut spans: Vec<Span> = Vec::new();

    // Profile badge
    spans.push(Span::styled(
        format!(" {} ", info.profile),
        Style::default()
            .bg(Color::Cyan)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::raw(" "));

    // Permission mode badge
    spans.push(Span::styled(
        format!(" {} ", info.permission_mode_label()),
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::raw(" "));

    // Session count
    spans.push(Span::styled(
        format!("sessions: {}", info.session_count),
        Style::default(),
    ));

    if !info.session_metadata.is_empty() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            info.session_metadata.join(" · "),
            Style::default().fg(Color::DarkGray),
        ));
    }

    spans.push(Span::raw("  "));

    // MCP server count (only shown if > 0)
    if info.mcp_server_count > 0 {
        spans.push(Span::styled(
            format!("MCP:{}↑", info.mcp_server_count),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("  "));
    }

    // Hint (dim)
    if !info.hint.is_empty() {
        spans.push(Span::styled(
            &info.hint,
            Style::default().add_modifier(Modifier::DIM),
        ));
    }

    // Error (red, bold) — prepend separator
    if let Some(err) = &info.error {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            err,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(notification) = notification.filter(|value| !value.is_empty()) {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            notification,
            status_notification_style(notification),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}

fn status_notification_style(message: &str) -> Style {
    if message.starts_with('[') && message.contains("error") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    }
}

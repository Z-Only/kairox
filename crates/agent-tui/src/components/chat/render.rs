use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::components::QueuedMessage;

/// Render the message list from a [`SessionProjection`] into the given area.
///
/// - User messages are prefixed with a cyan `"You:"`.
/// - Assistant messages are prefixed with a green `"Agent:"`.
/// - If the session was cancelled, a yellow `[cancelled]` line is shown.
/// - If `token_stream` is non-empty, the streaming text is shown with a `▌`
///   block cursor appended.
pub fn render_messages(
    area: Rect,
    frame: &mut Frame,
    projection: &agent_core::projection::SessionProjection,
) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in &projection.messages {
        let (label, color) = match msg.role {
            agent_core::projection::ProjectedRole::User => ("You:", Color::Cyan),
            agent_core::projection::ProjectedRole::Assistant => ("Agent:", Color::Green),
        };

        let content_lines: Vec<&str> = msg.content.split('\n').collect();
        for (i, line) in content_lines.iter().enumerate() {
            if i == 0 {
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", label), Style::default().fg(color)),
                    Span::raw(*line),
                ]));
            } else {
                lines.push(Line::raw(*line));
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

/// Render a one-line queue strip summarising messages waiting to be sent.
/// Returns silently when the queue is empty.
pub fn render_queue_strip(area: Rect, frame: &mut Frame, queue: &[QueuedMessage]) {
    if queue.is_empty() {
        return;
    }

    let count = queue.len();
    let preview = queue.first().map(|m| m.content.as_str()).unwrap_or("");
    // Keep the preview short — composer space is tight.
    let preview_trimmed: String = preview.chars().take(40).collect();
    let preview_display = if preview.chars().count() > 40 {
        format!("{preview_trimmed}…")
    } else {
        preview_trimmed
    };

    let label = if count == 1 {
        format!("⏳ 1 message queued: {preview_display}")
    } else {
        format!("⏳ {count} messages queued, next: {preview_display}")
    };

    let line = Line::from(vec![Span::styled(
        label,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::DIM),
    )]);
    frame.render_widget(Paragraph::new(line), area);
}

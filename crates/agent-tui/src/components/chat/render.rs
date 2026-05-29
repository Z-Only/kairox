use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use agent_core::AttachmentInfo;

use crate::components::{theme, QueuedMessage};

/// Render the message list from a [`SessionProjection`] into the given area.
///
/// - User messages are prefixed with an accent `"You:"`.
/// - Assistant messages are prefixed with a success `"Agent:"`.
/// - If the session was cancelled, a warning `[cancelled]` line is shown.
/// - If `token_stream` is non-empty, the streaming text is shown with a `▌`
///   block cursor appended.
///
/// As of v0.30.0 the binary path uses [`render_chat_stream`](super::render_chat_stream)
/// instead; this messages-only helper is retained for snapshot tests under
/// `crates/agent-tui/tests/` that pin pre-stream rendering behaviour. New
/// rendering wiring should target `render_chat_stream`.
#[allow(dead_code)]
pub fn render_messages(
    area: Rect,
    frame: &mut Frame,
    projection: &agent_core::projection::SessionProjection,
) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in &projection.messages {
        let (label, color) = match msg.role {
            agent_core::projection::ProjectedRole::User => ("You:", theme::ACCENT),
            agent_core::projection::ProjectedRole::Assistant => ("Agent:", theme::SUCCESS),
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
            Span::styled("Agent: ", Style::default().fg(theme::SUCCESS)),
            Span::raw(stream_text),
        ]));
    }

    if projection.cancelled {
        lines.push(Line::from(Span::styled(
            "[cancelled]",
            Style::default().fg(theme::WARNING),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Render a compact queue strip showing the first queued messages and the
/// selected row. Returns silently when the queue is empty.
pub fn render_queue_strip(
    area: Rect,
    frame: &mut Frame,
    queue: &[QueuedMessage],
    selected_index: Option<usize>,
) {
    if queue.is_empty() {
        return;
    }

    let selected = selected_index.unwrap_or(0).min(queue.len() - 1);
    let max_rows = area.height.saturating_sub(1).max(1) as usize;
    let start = selected.saturating_sub(max_rows.saturating_sub(1));
    let visible = queue
        .iter()
        .enumerate()
        .skip(start)
        .take(max_rows)
        .map(|(idx, message)| {
            let marker = if idx == selected { ">" } else { " " };
            let style = if idx == selected {
                Style::default()
                    .fg(theme::WARNING)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(theme::WARNING)
                    .add_modifier(Modifier::DIM)
            };
            Line::from(vec![Span::styled(
                format!("{marker} Q{} {}", idx + 1, queued_message_preview(message)),
                style,
            )])
        });

    let mut lines: Vec<Line> = visible.collect();
    let hint = if queue.len() == 1 {
        "1 queued | Alt+Enter send | :queue edit/delete".to_string()
    } else {
        format!(
            "{} queued | Alt+Up/Down select | Alt+Left/Right reorder | :queue send/edit/delete",
            queue.len()
        )
    };
    lines.push(Line::from(Span::styled(
        hint,
        theme::muted().add_modifier(Modifier::DIM),
    )));
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn render_file_mention_palette(
    area: Rect,
    frame: &mut Frame,
    matches: &[String],
    selected_index: Option<usize>,
) {
    let mut lines = Vec::new();
    if matches.is_empty() {
        lines.push(Line::from(Span::styled(
            "No file matches",
            theme::muted().add_modifier(Modifier::DIM),
        )));
    } else {
        let selected = selected_index.unwrap_or(0).min(matches.len() - 1);
        let max_rows = area.height.saturating_sub(1).max(1) as usize;
        let start = selected.saturating_sub(max_rows.saturating_sub(1));
        lines.extend(
            matches
                .iter()
                .enumerate()
                .skip(start)
                .take(max_rows)
                .map(|(idx, path)| {
                    let marker = if idx == selected { ">" } else { " " };
                    let style = if idx == selected {
                        Style::default()
                            .fg(theme::ACCENT)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        theme::muted()
                    };
                    Line::from(Span::styled(format!("{marker} @{path}"), style))
                }),
        );
    }
    lines.push(Line::from(Span::styled(
        "Enter attach | Up/Down select | Esc close",
        theme::muted().add_modifier(Modifier::DIM),
    )));
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn format_attachment_labels(attachments: &[AttachmentInfo]) -> String {
    if attachments.is_empty() {
        return String::new();
    }

    let mut labels: Vec<String> = attachments
        .iter()
        .take(2)
        .map(|attachment| format!("[{}]", truncate_chars(&attachment.name, 18)))
        .collect();
    if attachments.len() > 2 {
        labels.push(format!("[+{}]", attachments.len() - 2));
    }
    labels.join(" ")
}

fn queued_message_preview(message: &QueuedMessage) -> String {
    let content = truncate_chars(message.content.as_str(), 40);
    let labels = format_attachment_labels(&message.attachments);
    match (content.is_empty(), labels.is_empty()) {
        (true, true) => String::new(),
        (true, false) => labels,
        (false, true) => content,
        (false, false) => format!("{content} {labels}"),
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

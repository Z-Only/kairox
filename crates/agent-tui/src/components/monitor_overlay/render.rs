//! Rendering helpers for the monitor overlay.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::theme;

use super::state::MonitorOverlay;
use super::types::MonitorEntry;

pub fn render_monitor_overlay(
    area: Rect,
    frame: &mut Frame,
    overlay: &MonitorOverlay,
    list_state: &mut ListState,
) {
    let modal_width = 88.min(area.width.saturating_sub(4));
    let modal_height = 20.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Monitor Manager ", theme::title()))
        .border_style(theme::border(true));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    if inner.height < 4 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    render_header(chunks[0], frame, &overlay.monitors);
    render_monitors(chunks[1], frame, &overlay.monitors, list_state);
    render_hints(chunks[2], frame);
}

fn render_header(area: Rect, frame: &mut Frame, monitors: &[MonitorEntry]) {
    let spans = vec![
        Span::styled(" Active Monitors ", theme::title()),
        Span::styled(
            format!("({})", monitors.len()),
            Style::default().fg(theme::ACCENT),
        ),
    ];
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_monitors(
    area: Rect,
    frame: &mut Frame,
    monitors: &[MonitorEntry],
    state: &mut ListState,
) {
    if monitors.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No active monitors",
                theme::muted(),
            ))),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = monitors
        .iter()
        .map(|m| {
            let persistence = if m.persistent {
                "persistent"
            } else {
                &format_timeout(m.timeout_ms)
            };
            let line = Line::from(vec![
                Span::styled(
                    m.monitor_id.clone(),
                    Style::default()
                        .fg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    m.description.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    truncate_command(&m.command, 30),
                    theme::muted(),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("[{persistence}]"),
                    theme::muted(),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).highlight_style(theme::selected());
    frame.render_stateful_widget(list, area, state);
}

fn render_hints(area: Rect, frame: &mut Frame) {
    let hints = Line::from(vec![
        Span::styled("[j/k] nav  ", theme::muted()),
        Span::styled("[x] stop  ", theme::key()),
        Span::styled("[r] refresh  ", theme::title()),
        Span::styled("[Esc] close", theme::muted()),
    ]);
    frame.render_widget(Paragraph::new(hints), area);
}

fn format_timeout(ms: u64) -> String {
    if ms >= 60_000 {
        format!("{}m timeout", ms / 60_000)
    } else {
        format!("{}s timeout", ms / 1_000)
    }
}

fn truncate_command(cmd: &str, max_len: usize) -> String {
    if cmd.len() <= max_len {
        cmd.to_string()
    } else {
        format!("{}...", &cmd[..max_len.saturating_sub(3)])
    }
}

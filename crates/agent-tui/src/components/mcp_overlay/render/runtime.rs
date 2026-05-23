use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, ListState};
use ratatui::Frame;

use crate::components::McpServerStatusView;

use super::super::state::McpOverlay;
use super::{clip, render_empty, render_list};

pub(super) fn render_runtime(
    area: Rect,
    frame: &mut Frame,
    overlay: &McpOverlay,
    state: &mut ListState,
) {
    let servers = &overlay.runtime_servers;
    if servers.is_empty() {
        render_empty(area, frame, "No MCP runtime servers configured");
        return;
    }
    let items: Vec<ListItem> = servers
        .iter()
        .map(|s| {
            let (status_label, status_color) = match s.status {
                McpServerStatusView::Running => ("running ", Color::Green),
                McpServerStatusView::Starting => ("starting", Color::Yellow),
                McpServerStatusView::Stopped => ("stopped ", Color::Gray),
                McpServerStatusView::Failed => ("failed  ", Color::Red),
            };
            let trust_label = if s.trusted { " trusted" } else { "" };
            let health = overlay.health.get(&s.server_id);
            let health_label = health
                .map(|state| {
                    if state.healthy {
                        format!(" health:ok({})", state.tool_count)
                    } else if let Some(error) = &state.error {
                        format!(" health:fail({})", clip(error, 18))
                    } else {
                        " health:fail".to_string()
                    }
                })
                .unwrap_or_default();
            let health_color = match health {
                Some(state) if state.healthy => Color::Green,
                Some(_) => Color::Red,
                None => Color::DarkGray,
            };
            let connectivity = overlay.connectivity.get(&s.server_id);
            let connectivity_label = connectivity
                .map(|state| {
                    if state.connected {
                        let count = state
                            .tool_count
                            .map(|tool_count| format!("({tool_count})"))
                            .unwrap_or_default();
                        format!(" conn:ok{count}")
                    } else {
                        " conn:fail".to_string()
                    }
                })
                .unwrap_or_default();
            let connectivity_color = match connectivity {
                Some(state) if state.connected => Color::Green,
                Some(_) => Color::Red,
                None => Color::DarkGray,
            };
            ListItem::new(Line::from(vec![
                Span::styled(status_label, Style::default().fg(status_color)),
                Span::raw("  "),
                Span::styled(
                    s.server_id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  tools:{}", s.tool_count),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(trust_label, Style::default().fg(Color::Magenta)),
                Span::styled(health_label, Style::default().fg(health_color)),
                Span::styled(connectivity_label, Style::default().fg(connectivity_color)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

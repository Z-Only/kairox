use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, ListState};
use ratatui::Frame;

use crate::components::{theme, McpServerStatusView};

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
                McpServerStatusView::Running => ("running ", theme::SUCCESS),
                McpServerStatusView::Starting => ("starting", theme::WARNING),
                McpServerStatusView::Stopped => ("stopped ", theme::MUTED),
                McpServerStatusView::Failed => ("failed  ", theme::DANGER),
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
                Some(state) if state.healthy => theme::SUCCESS,
                Some(_) => theme::DANGER,
                None => theme::MUTED,
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
                Some(state) if state.connected => theme::SUCCESS,
                Some(_) => theme::DANGER,
                None => theme::MUTED,
            };
            let disabled_count = overlay
                .tools
                .get(&s.server_id)
                .map(|tools| tools.iter().filter(|t| t.disabled).count())
                .unwrap_or(0);
            let tools_label = if disabled_count > 0 {
                format!("  tools:{} ({disabled_count} off)", s.tool_count)
            } else {
                format!("  tools:{}", s.tool_count)
            };
            let tools_color = if disabled_count > 0 {
                theme::WARNING
            } else {
                theme::MUTED
            };
            ListItem::new(Line::from(vec![
                Span::styled(status_label, Style::default().fg(status_color)),
                Span::raw("  "),
                Span::styled(
                    s.server_id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(tools_label, Style::default().fg(tools_color)),
                Span::styled(trust_label, Style::default().fg(theme::ACCENT_STRONG)),
                Span::styled(health_label, Style::default().fg(health_color)),
                Span::styled(connectivity_label, Style::default().fg(connectivity_color)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

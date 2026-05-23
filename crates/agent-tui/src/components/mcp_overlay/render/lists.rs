use agent_core::facade::{CatalogSourceView, InstalledEntry, McpServerSettingsView};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, ListState};
use ratatui::Frame;

use super::{render_empty, render_list};

pub(super) fn render_settings(
    area: Rect,
    frame: &mut Frame,
    settings: &[McpServerSettingsView],
    state: &mut ListState,
) {
    if settings.is_empty() {
        render_empty(area, frame, "No MCP server settings configured");
        return;
    }
    let items: Vec<ListItem> = settings
        .iter()
        .map(|setting| {
            let enabled_label = if setting.enabled {
                "enabled "
            } else {
                "disabled"
            };
            let enabled_color = if setting.enabled {
                Color::Green
            } else {
                Color::DarkGray
            };
            let writable = if setting.writable {
                " writable"
            } else {
                " read-only"
            };
            let tools = setting
                .tool_count
                .map(|count| format!(" tools:{count}"))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(enabled_label, Style::default().fg(enabled_color)),
                Span::raw("  "),
                Span::styled(
                    setting.id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", setting.runtime_status),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(
                    format!("  [{}{}]", setting.source, writable),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(tools, Style::default().fg(Color::Cyan)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

pub(super) fn render_installed(
    area: Rect,
    frame: &mut Frame,
    installed: &[InstalledEntry],
    state: &mut ListState,
) {
    if installed.is_empty() {
        render_empty(area, frame, "No MCP marketplace servers installed");
        return;
    }
    let items: Vec<ListItem> = installed
        .iter()
        .map(|entry| {
            let running = if entry.running { "running" } else { "stopped" };
            let source = entry.source.as_deref().unwrap_or("manual");
            let catalog = entry.catalog_id.as_deref().unwrap_or("unknown");
            ListItem::new(Line::from(vec![
                Span::styled(
                    running,
                    Style::default().fg(if entry.running {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
                ),
                Span::raw("  "),
                Span::styled(
                    entry.server_id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {catalog}@{source}"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

pub(super) fn render_sources(
    area: Rect,
    frame: &mut Frame,
    sources: &[CatalogSourceView],
    state: &mut ListState,
) {
    if sources.is_empty() {
        render_empty(area, frame, "No MCP catalog sources configured");
        return;
    }
    let items: Vec<ListItem> = sources
        .iter()
        .map(|source| {
            let enabled_label = if source.enabled {
                "enabled "
            } else {
                "disabled"
            };
            let enabled_color = if source.enabled {
                Color::Green
            } else {
                Color::DarkGray
            };
            let location = if source.url.is_empty() {
                "builtin".to_string()
            } else {
                source.url.clone()
            };
            ListItem::new(Line::from(vec![
                Span::styled(enabled_label, Style::default().fg(enabled_color)),
                Span::raw("  "),
                Span::styled(
                    source.id.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", source.kind),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(location, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

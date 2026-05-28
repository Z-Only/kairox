use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, ListState};
use ratatui::Frame;

use super::super::state::McpOverlay;
use super::super::types::resource_preview_key;
use super::{clip, render_empty, render_list};

pub(super) fn render_tools(
    area: Rect,
    frame: &mut Frame,
    overlay: &McpOverlay,
    state: &mut ListState,
) {
    let Some(server_id) = overlay.selected_server_id() else {
        render_empty(area, frame, "Select a runtime server before browsing tools");
        return;
    };
    let tools = overlay.current_tools();
    if tools.is_empty() {
        let label = if overlay.health.contains_key(server_id) {
            "No MCP tools discovered for selected server"
        } else {
            "Press [r] to health-check selected server and load tools"
        };
        render_empty(area, frame, label);
        return;
    }
    let items: Vec<ListItem> = tools
        .iter()
        .map(|tool| {
            let state_label = if tool.disabled {
                "disabled"
            } else {
                "enabled "
            };
            let state_color = if tool.disabled {
                Color::DarkGray
            } else {
                Color::Green
            };
            let description = tool
                .description
                .as_ref()
                .map(|value| format!("  {}", clip(value, 56)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(state_label, Style::default().fg(state_color)),
                Span::raw("  "),
                Span::styled(
                    tool.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(description, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

pub(super) fn render_resources(
    area: Rect,
    frame: &mut Frame,
    overlay: &McpOverlay,
    state: &mut ListState,
) {
    if overlay.selected_server_id().is_none() {
        render_empty(
            area,
            frame,
            "Select a runtime server before browsing resources",
        );
        return;
    }
    let resources = overlay.current_resources();
    if resources.is_empty() {
        render_empty(
            area,
            frame,
            "Press [r] to list resources for selected server",
        );
        return;
    }
    let items: Vec<ListItem> = resources
        .iter()
        .map(|resource| {
            let mime = resource
                .mime_type
                .as_ref()
                .map(|value| format!("  {value}"))
                .unwrap_or_default();
            let preview = overlay
                .resource_previews
                .get(&resource_preview_key(&resource.server_id, &resource.uri))
                .map(|value| format!("  {}", clip(value, 56)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(
                    resource.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", clip(&resource.uri, 42)),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(mime, Style::default().fg(Color::DarkGray)),
                Span::styled(preview, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

pub(super) fn render_prompts(
    area: Rect,
    frame: &mut Frame,
    overlay: &McpOverlay,
    state: &mut ListState,
) {
    if overlay.selected_server_id().is_none() {
        render_empty(
            area,
            frame,
            "Select a runtime server before browsing prompts",
        );
        return;
    }
    let prompts = overlay.current_prompts();
    if prompts.is_empty() {
        render_empty(area, frame, "Press [r] to list prompts for selected server");
        return;
    }
    let items: Vec<ListItem> = prompts
        .iter()
        .map(|prompt| {
            let description = prompt
                .description
                .as_ref()
                .map(|value| format!("  {}", clip(value, 56)))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(
                    prompt.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  args:{}", prompt.argument_count),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(description, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect();
    render_list(area, frame, items, state);
}

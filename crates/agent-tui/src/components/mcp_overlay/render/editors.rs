use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Frame;

use super::super::editor::{
    ServerDraft, ServerEditorField, ServerTransportDraft, SourceDraft, SourceEditorField,
    SERVER_EDITOR_FIELDS, SOURCE_EDITOR_FIELDS,
};
use super::super::state::McpOverlay;
use super::clip;

pub(super) fn render_server_editor(area: Rect, frame: &mut Frame, overlay: &McpOverlay) {
    let list_height = area.height.saturating_sub(1);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );
    let items = SERVER_EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let marker = if index == overlay.server_field_index {
                "> "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Magenta)),
                Span::styled(
                    format!("{:<12}", server_field_label(*field)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    server_field_value(&overlay.server_draft, *field),
                    Style::default().fg(Color::Gray),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), list_area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "[Tab/Up/Down] field  ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("[s/e/h] transport  ", Style::default().fg(Color::Cyan)),
            Span::styled("[space/y/n] enabled  ", Style::default().fg(Color::Green)),
            Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
            Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
        ])),
        hint_area,
    );
}

pub(super) fn render_source_editor(area: Rect, frame: &mut Frame, overlay: &McpOverlay) {
    let list_height = area.height.saturating_sub(1);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );
    let items = SOURCE_EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let marker = if index == overlay.source_field_index {
                "> "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Magenta)),
                Span::styled(
                    format!("{:<12}", source_field_label(*field)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    source_field_value(&overlay.source_draft, *field),
                    Style::default().fg(Color::Gray),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), list_area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "[Tab/Up/Down] field  ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("[space/y/n] enabled  ", Style::default().fg(Color::Green)),
            Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
            Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
        ])),
        hint_area,
    );
}

pub(super) fn render_catalog_install_config_editor(
    area: Rect,
    frame: &mut Frame,
    overlay: &McpOverlay,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    let draft = &overlay.catalog_install_draft;

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "Install configuration: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(clip(&draft.display_name, 64)),
            ]),
            Line::from(Span::styled(
                "Fill required MCP catalog values before installing",
                Style::default().fg(Color::DarkGray),
            )),
        ]),
        chunks[0],
    );

    let items = draft
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let marker = if index == overlay.catalog_install_field_index {
                "> "
            } else {
                "  "
            };
            let value = draft.values.get(&item.key).cloned().unwrap_or_default();
            let missing = item.required && value.trim().is_empty();
            let value_label = if item.secret && !value.is_empty() {
                "*".repeat(value.chars().count().min(12))
            } else {
                value
            };
            let required = if item.required {
                "required"
            } else {
                "optional"
            };
            let required_color = if missing {
                Color::Yellow
            } else {
                Color::DarkGray
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Magenta)),
                Span::styled(
                    format!("{:<18}", item.key),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<12}", item.kind),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{required:<8} "),
                    Style::default().fg(required_color),
                ),
                Span::styled(
                    if item.secret { "secret " } else { "       " },
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(value_label),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), chunks[1]);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "[Tab/Up/Down] field  ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled("[Enter] install  ", Style::default().fg(Color::Yellow)),
            Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
        ])),
        chunks[2],
    );
}

fn server_field_label(field: ServerEditorField) -> &'static str {
    match field {
        ServerEditorField::Name => "Name",
        ServerEditorField::Transport => "Transport",
        ServerEditorField::CommandOrUrl => "Command/URL",
        ServerEditorField::Args => "Args",
        ServerEditorField::Description => "Description",
        ServerEditorField::Enabled => "Enabled",
    }
}

fn server_field_value(draft: &ServerDraft, field: ServerEditorField) -> String {
    match field {
        ServerEditorField::Name => draft.name.clone(),
        ServerEditorField::Transport => server_transport_label(draft.transport).to_string(),
        ServerEditorField::CommandOrUrl if draft.transport == ServerTransportDraft::Stdio => {
            draft.command.clone()
        }
        ServerEditorField::CommandOrUrl => draft.url.clone(),
        ServerEditorField::Args => {
            if draft.transport == ServerTransportDraft::Stdio {
                draft.args_text.clone()
            } else {
                "n/a".to_string()
            }
        }
        ServerEditorField::Description => draft.description.clone(),
        ServerEditorField::Enabled => draft.enabled.to_string(),
    }
}

fn source_field_label(field: SourceEditorField) -> &'static str {
    match field {
        SourceEditorField::Id => "ID",
        SourceEditorField::DisplayName => "Name",
        SourceEditorField::Url => "URL",
        SourceEditorField::ApiKeyEnv => "API key env",
        SourceEditorField::Priority => "Priority",
        SourceEditorField::DefaultTrust => "Trust",
        SourceEditorField::Enabled => "Enabled",
    }
}

fn source_field_value(draft: &SourceDraft, field: SourceEditorField) -> String {
    match field {
        SourceEditorField::Id => draft.id.clone(),
        SourceEditorField::DisplayName => draft.display_name.clone(),
        SourceEditorField::Url => draft.url.clone(),
        SourceEditorField::ApiKeyEnv => draft.api_key_env.clone(),
        SourceEditorField::Priority => draft.priority.clone(),
        SourceEditorField::DefaultTrust => draft.default_trust.clone(),
        SourceEditorField::Enabled => draft.enabled.to_string(),
    }
}

fn server_transport_label(transport: ServerTransportDraft) -> &'static str {
    match transport {
        ServerTransportDraft::Stdio => "stdio",
        ServerTransportDraft::Sse => "sse",
        ServerTransportDraft::StreamableHttp => "streamable_http",
    }
}

//! Rendering helpers for [`AgentOverlay`].
//!
//! These functions read state owned by `super::state::AgentOverlay` and lay
//! out the modal, list rows, and editor form. They never mutate state — the
//! Component implementation in [`super`] is responsible for that.

use agent_core::facade::AgentSettingsScope;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use super::state::{AgentDraft, AgentEditorField, AgentOverlay, AgentOverlayMode, EDITOR_FIELDS};

fn scope_label(scope: AgentSettingsScope) -> &'static str {
    match scope {
        AgentSettingsScope::Builtin => "builtin",
        AgentSettingsScope::User => "user",
        AgentSettingsScope::Project => "project",
    }
}

fn scope_color(scope: AgentSettingsScope) -> Color {
    match scope {
        AgentSettingsScope::Builtin => Color::DarkGray,
        AgentSettingsScope::User => Color::Cyan,
        AgentSettingsScope::Project => Color::Magenta,
    }
}

fn editor_field_label(field: AgentEditorField) -> &'static str {
    match field {
        AgentEditorField::Scope => "Scope",
        AgentEditorField::Name => "Name",
        AgentEditorField::Description => "Description",
        AgentEditorField::Tools => "Tools",
        AgentEditorField::ModelProfile => "Model",
        AgentEditorField::PermissionMode => "Permission",
        AgentEditorField::Skills => "Skills",
        AgentEditorField::Nicknames => "Nicknames",
        AgentEditorField::Enabled => "Enabled",
        AgentEditorField::Instructions => "Instructions",
    }
}

fn editor_field_value(draft: &AgentDraft, field: AgentEditorField) -> String {
    match field {
        AgentEditorField::Scope => scope_label(draft.scope).to_string(),
        AgentEditorField::Name => draft.name.clone(),
        AgentEditorField::Description => draft.description.clone(),
        AgentEditorField::Tools => draft.tools_text.clone(),
        AgentEditorField::ModelProfile => draft.model_profile.clone(),
        AgentEditorField::PermissionMode => draft.permission_mode.clone(),
        AgentEditorField::Skills => draft.skills_text.clone(),
        AgentEditorField::Nicknames => draft.nicknames_text.clone(),
        AgentEditorField::Enabled => draft.enabled.to_string(),
        AgentEditorField::Instructions => draft.instructions.clone(),
    }
}

pub fn render_agent_overlay(area: Rect, frame: &mut Frame, overlay: &AgentOverlay) {
    let modal_width = 108.min(area.width.saturating_sub(4));
    let modal_height = 24.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let title = match overlay.mode {
        AgentOverlayMode::List => " Agent Settings ",
        AgentOverlayMode::Editor => " Agent Settings Editor ",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    match overlay.mode {
        AgentOverlayMode::List => render_agent_list(inner, frame, overlay),
        AgentOverlayMode::Editor => render_agent_editor(inner, frame, overlay),
    }
}

fn render_agent_list(area: Rect, frame: &mut Frame, overlay: &AgentOverlay) {
    let list_height = area.height.saturating_sub(2);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );

    if overlay.agents.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No agent profiles configured",
                Style::default().fg(Color::DarkGray),
            ))),
            list_area,
        );
    } else {
        let items = overlay
            .agents
            .iter()
            .map(|agent| {
                let effective_marker = if agent.effective { "* " } else { "  " };
                let state = if agent.enabled { "enabled" } else { "disabled" };
                let validity = if agent.valid { "" } else { " invalid" };
                let edit = if agent.editable {
                    " editable"
                } else {
                    " read-only"
                };
                ListItem::new(Line::from(vec![
                    Span::styled(effective_marker, Style::default().fg(Color::Green)),
                    Span::styled(
                        format!("{:<7}", scope_label(agent.scope)),
                        Style::default().fg(scope_color(agent.scope)),
                    ),
                    Span::styled(
                        format!(" {:<8}", state),
                        Style::default().fg(if agent.enabled {
                            Color::Green
                        } else {
                            Color::DarkGray
                        }),
                    ),
                    Span::styled(
                        format!(" {}", agent.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" - {}", agent.description),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(
                        format!("{edit}{validity}"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        let list = List::new(items).highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
        let mut state = overlay.list_state;
        frame.render_stateful_widget(list, list_area, &mut state);
    }

    let hints = Line::from(vec![
        Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[n/N] new user/project  ", Style::default().fg(Color::Cyan)),
        Span::styled("[Enter/e] edit  ", Style::default().fg(Color::Yellow)),
        Span::styled(
            "[c/p] copy user/project  ",
            Style::default().fg(Color::Green),
        ),
        Span::styled("[x] delete  ", Style::default().fg(Color::Red)),
        Span::styled("[o] folder  ", Style::default().fg(Color::Blue)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

fn render_agent_editor(area: Rect, frame: &mut Frame, overlay: &AgentOverlay) {
    let list_height = area.height.saturating_sub(2);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );

    let items = EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let marker = if index == overlay.editor_field_index {
                "> "
            } else {
                "  "
            };
            let value = editor_field_value(&overlay.draft, *field);
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{:<12}", editor_field_label(*field)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect::<Vec<_>>();
    let list = List::new(items);
    frame.render_widget(list, list_area);

    let hints = Line::from(vec![
        Span::styled("[Tab/j/k] field  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[u/p] scope  ", Style::default().fg(Color::Cyan)),
        Span::styled("[space/y/n] enabled  ", Style::default().fg(Color::Green)),
        Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

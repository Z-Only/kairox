//! Model overlay rendering — pure visual layer reading [`ModelOverlay`] state.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::state::{
    ModelOverlay, OverlayFocus, OverlayMode, ProfileDraft, ProfileEditorField,
    PROFILE_EDITOR_FIELDS, REASONING_EFFORTS,
};

pub fn render_model_overlay(
    area: Rect,
    frame: &mut Frame,
    overlay: &ModelOverlay,
    list_state: &mut ListState,
    effort_state: &mut ListState,
) {
    let modal_width = 96.min(area.width.saturating_sub(4));
    let modal_height = 22.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            match overlay.mode {
                OverlayMode::List => " 🤖 Model Profile ",
                OverlayMode::Editor => " 🤖 Model Profile Editor ",
            },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    match overlay.mode {
        OverlayMode::List => {
            render_model_profile_list(inner, frame, overlay, list_state, effort_state);
        }
        OverlayMode::Editor => render_model_profile_editor(inner, frame, overlay),
    }
}

fn render_model_profile_list(
    inner: Rect,
    frame: &mut Frame,
    overlay: &ModelOverlay,
    list_state: &mut ListState,
    effort_state: &mut ListState,
) {
    let list_height = inner.height.saturating_sub(2);
    let list_area = Rect::new(inner.x, inner.y, inner.width, list_height);
    let hint_area = Rect::new(
        inner.x,
        inner.y + list_height,
        inner.width,
        inner.height.saturating_sub(list_height),
    );

    let show_effort = overlay.shows_effort_picker();
    let columns = if show_effort {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(list_area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(list_area)
    };

    if overlay.profiles.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No model profiles configured",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, columns[0]);
    } else {
        let items: Vec<ListItem> = overlay
            .profiles
            .iter()
            .map(|p| {
                let is_current = overlay.current_alias.as_deref() == Some(p.alias.as_str());
                let marker = if is_current { "● " } else { "  " };
                let enabled_label = if p.enabled { "enabled " } else { "disabled" };
                let enabled_color = if p.enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                };
                let reasoning_tag = if p.supports_reasoning { " [R]" } else { "" };
                let writable_tag = if p.writable {
                    " writable"
                } else {
                    " read-only"
                };
                let key_tag = if p.has_api_key { " key" } else { " no-key" };
                let test_tag = overlay
                    .test_results
                    .get(&p.alias)
                    .map(|result| {
                        if result.ok {
                            " test:ok".to_string()
                        } else {
                            result
                                .message
                                .as_deref()
                                .map(|message| format!(" test:{message}"))
                                .unwrap_or_else(|| " test:failed".to_string())
                        }
                    })
                    .unwrap_or_default();
                let line = Line::from(vec![
                    Span::styled(marker, Style::default().fg(Color::Green)),
                    Span::styled(enabled_label, Style::default().fg(enabled_color)),
                    Span::raw("  "),
                    Span::styled(
                        p.alias.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {}/{}", p.provider_display, p.model_display),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(reasoning_tag, Style::default().fg(Color::Magenta)),
                    Span::styled(
                        format!("  [{}{writable_tag}{key_tag}{test_tag}]", p.source),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let highlight = if overlay.overlay_focus == OverlayFocus::ProfileList {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::Reset)
        };
        let list = List::new(items).highlight_style(highlight);
        frame.render_stateful_widget(list, columns[0], list_state);
    }

    if show_effort {
        let items: Vec<ListItem> = REASONING_EFFORTS
            .iter()
            .map(|effort| {
                let is_current = overlay.current_effort.as_deref() == Some(*effort);
                let marker = if is_current { "● " } else { "  " };
                let line = Line::from(vec![
                    Span::styled(marker, Style::default().fg(Color::Green)),
                    Span::raw(*effort),
                ]);
                ListItem::new(line)
            })
            .collect();
        let highlight = if overlay.overlay_focus == OverlayFocus::EffortList {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::Reset)
        };
        let effort_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                " effort ",
                Style::default().fg(Color::Magenta),
            ));
        let effort_inner = effort_block.inner(columns[1]);
        frame.render_widget(effort_block, columns[1]);
        let list = List::new(items).highlight_style(highlight);
        frame.render_stateful_widget(list, effort_inner, effort_state);
    }

    let hints = Line::from(vec![
        Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[n] new  ", Style::default().fg(Color::Cyan)),
        Span::styled("[u] edit  ", Style::default().fg(Color::Yellow)),
        Span::styled("[J/K] order  ", Style::default().fg(Color::Cyan)),
        Span::styled("[e] enable  ", Style::default().fg(Color::Green)),
        Span::styled("[t] test  ", Style::default().fg(Color::Yellow)),
        Span::styled("[x] delete  ", Style::default().fg(Color::Red)),
        Span::styled("[o] config  ", Style::default().fg(Color::Blue)),
        Span::styled("[Tab] effort  ", Style::default().fg(Color::Magenta)),
        Span::styled("[Enter] switch  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

fn render_model_profile_editor(area: Rect, frame: &mut Frame, overlay: &ModelOverlay) {
    let list_height = area.height.saturating_sub(2);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );

    let items = PROFILE_EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let marker = if index == overlay.editor_field_index {
                "> "
            } else {
                "  "
            };
            let label = profile_editor_field_label(*field);
            let value = profile_editor_field_value(&overlay.draft, *field);
            let lock_hint = if *field == ProfileEditorField::Alias && !overlay.draft.alias_editable
            {
                " (locked)"
            } else {
                ""
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{label:<14}"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(Color::Gray)),
                Span::styled(lock_hint, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), list_area);

    let hints = Line::from(vec![
        Span::styled("[Tab/j/k] field  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[space/y/n] enabled  ", Style::default().fg(Color::Green)),
        Span::styled("[Ctrl+T] test URL  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

fn profile_editor_field_label(field: ProfileEditorField) -> &'static str {
    match field {
        ProfileEditorField::Alias => "Alias",
        ProfileEditorField::Provider => "Provider",
        ProfileEditorField::ModelId => "Model ID",
        ProfileEditorField::BaseUrl => "Base URL",
        ProfileEditorField::ApiKeyEnv => "API key env",
        ProfileEditorField::ContextWindow => "Context",
        ProfileEditorField::OutputLimit => "Output",
        ProfileEditorField::Temperature => "Temperature",
        ProfileEditorField::TopP => "Top P",
        ProfileEditorField::TopK => "Top K",
        ProfileEditorField::MaxTokens => "Max tokens",
        ProfileEditorField::Enabled => "Enabled",
    }
}

fn profile_editor_field_value(draft: &ProfileDraft, field: ProfileEditorField) -> String {
    match field {
        ProfileEditorField::Alias => draft.alias.clone(),
        ProfileEditorField::Provider => draft.provider.clone(),
        ProfileEditorField::ModelId => draft.model_id.clone(),
        ProfileEditorField::BaseUrl => draft.base_url.clone(),
        ProfileEditorField::ApiKeyEnv => draft.api_key_env.clone(),
        ProfileEditorField::ContextWindow => draft.context_window.clone(),
        ProfileEditorField::OutputLimit => draft.output_limit.clone(),
        ProfileEditorField::Temperature => draft.temperature.clone(),
        ProfileEditorField::TopP => draft.top_p.clone(),
        ProfileEditorField::TopK => draft.top_k.clone(),
        ProfileEditorField::MaxTokens => draft.max_tokens.clone(),
        ProfileEditorField::Enabled => draft.enabled.to_string(),
    }
}

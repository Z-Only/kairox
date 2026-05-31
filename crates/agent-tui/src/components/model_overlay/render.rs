//! Model overlay rendering — pure visual layer reading [`ModelOverlay`] state.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::theme;

use super::state::{ModelOverlay, REASONING_EFFORTS};
use super::types::{
    OverlayFocus, OverlayMode, ProfileDraft, ProfileEditorField, PROFILE_EDITOR_FIELDS,
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
                OverlayMode::List => " Model Profile ",
                OverlayMode::Editor => " Model Profile Editor ",
            },
            theme::title(),
        ))
        .border_style(theme::border(true));

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
            theme::muted(),
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
                    theme::SUCCESS
                } else {
                    theme::MUTED
                };
                let reasoning_tag = if p.supports_reasoning { " [R]" } else { "" };
                let limits_tag = format_context_limits(p.context_window, p.output_limit);
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
                    Span::styled(marker, Style::default().fg(theme::SUCCESS)),
                    Span::styled(enabled_label, Style::default().fg(enabled_color)),
                    Span::raw("  "),
                    Span::styled(
                        p.alias.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {}/{}", p.provider_display, p.model_display),
                        theme::muted(),
                    ),
                    Span::styled(reasoning_tag, Style::default().fg(theme::ACCENT_STRONG)),
                    Span::styled(
                        format!("  {limits_tag}"),
                        Style::default().fg(theme::INFO),
                    ),
                    Span::styled(
                        format!("  [{}{writable_tag}{key_tag}{test_tag}]", p.source),
                        theme::muted(),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let highlight = if overlay.overlay_focus == OverlayFocus::ProfileList {
            theme::selected()
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
                    Span::styled(marker, Style::default().fg(theme::SUCCESS)),
                    Span::raw(*effort),
                ]);
                ListItem::new(line)
            })
            .collect();
        let highlight = if overlay.overlay_focus == OverlayFocus::EffortList {
            theme::selected()
        } else {
            Style::default().bg(Color::Reset)
        };
        let effort_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(theme::border(false))
            .title(Span::styled(" effort ", theme::title()));
        let effort_inner = effort_block.inner(columns[1]);
        frame.render_widget(effort_block, columns[1]);
        let list = List::new(items).highlight_style(highlight);
        frame.render_stateful_widget(list, effort_inner, effort_state);
    }

    let hints = Line::from(vec![
        Span::styled("[j/k] nav  ", theme::muted()),
        Span::styled("[n] new  ", theme::title()),
        Span::styled("[u] edit  ", theme::key()),
        Span::styled("[J/K] order  ", theme::title()),
        Span::styled("[e] enable  ", Style::default().fg(theme::SUCCESS)),
        Span::styled("[t] test  ", theme::key()),
        Span::styled("[x] delete  ", Style::default().fg(theme::DANGER)),
        Span::styled("[o] config  ", Style::default().fg(theme::INFO)),
        Span::styled("[Tab] effort  ", Style::default().fg(theme::ACCENT_STRONG)),
        Span::styled("[Enter] switch  ", theme::key()),
        Span::styled("[Esc] close", theme::muted()),
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
                Span::styled(marker, theme::title()),
                Span::styled(
                    format!("{label:<14}"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(value, theme::muted()),
                Span::styled(lock_hint, theme::muted()),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), list_area);

    let hints = Line::from(vec![
        Span::styled("[Tab/j/k] field  ", theme::muted()),
        Span::styled("[space/y/n] enabled  ", Style::default().fg(theme::SUCCESS)),
        Span::styled("[Ctrl+T] test URL  ", theme::key()),
        Span::styled("[Enter] save  ", theme::key()),
        Span::styled("[Esc] cancel", theme::muted()),
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

/// Format a token count into a compact human-readable form (e.g. "128k", "1M").
pub(super) fn format_token_count(tokens: u64) -> String {
    if tokens >= 1_000_000 && tokens % 1_000_000 == 0 {
        format!("{}M", tokens / 1_000_000)
    } else if tokens >= 1_000 && tokens % 1_000 == 0 {
        format!("{}k", tokens / 1_000)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// Format context window and output limit into a compact label for the
/// profile list, e.g. `ctx:128k/8k`. Returns an empty string when both
/// values are absent.
pub(super) fn format_context_limits(context_window: Option<u64>, output_limit: Option<u64>) -> String {
    match (context_window, output_limit) {
        (Some(ctx), Some(out)) => {
            format!(
                "ctx:{}/{}",
                format_token_count(ctx),
                format_token_count(out)
            )
        }
        (Some(ctx), None) => format!("ctx:{}", format_token_count(ctx)),
        (None, Some(out)) => format!("out:{}", format_token_count(out)),
        (None, None) => String::new(),
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

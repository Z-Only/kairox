//! Rendering helpers for [`HooksOverlay`].
//!
//! These functions read state owned by `super::state::HooksOverlay` and lay
//! out the modal, list rows, and editor form. They never mutate state — the
//! Component implementation in [`super::mod`](super) is responsible for that.

use agent_core::facade::{HookSettingsView, HookTemplateView};
use agent_core::ConfigScope;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::theme;

use super::state::HooksOverlay;
use super::types::{HookDraft, HookEditorField, HooksMode, HooksTab, EDITOR_FIELDS};

pub(super) fn render_hooks_overlay(area: Rect, frame: &mut Frame, overlay: &HooksOverlay) {
    let modal_width = 112.min(area.width.saturating_sub(4));
    let modal_height = 26.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let title = match overlay.mode {
        HooksMode::List => " Hooks Settings ",
        HooksMode::Editor => " Hooks Settings Editor ",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, theme::title()))
        .border_style(theme::border(true));
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    match overlay.mode {
        HooksMode::List => render_hooks_list(inner, frame, overlay),
        HooksMode::Editor => render_hooks_editor(inner, frame, overlay),
    }
}

fn render_hooks_list(area: Rect, frame: &mut Frame, overlay: &HooksOverlay) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);

    frame.render_widget(Paragraph::new(render_tabs(overlay)), chunks[0]);

    match overlay.tab {
        HooksTab::User => render_hook_rows(chunks[1], frame, &overlay.user, overlay.user_state),
        HooksTab::Project => {
            render_hook_rows(chunks[1], frame, &overlay.project, overlay.project_state)
        }
        HooksTab::Templates => {
            render_template_rows(chunks[1], frame, &overlay.templates, overlay.template_state)
        }
    }

    let hints = if overlay.tab == HooksTab::Templates {
        Line::from(vec![
            Span::styled("[j/k] nav  ", theme::muted()),
            Span::styled("[Enter/u] use user  ", theme::title()),
            Span::styled(
                "[p] use project  ",
                Style::default().fg(theme::ACCENT_STRONG),
            ),
            Span::styled("[Tab] tab  ", theme::muted()),
            Span::styled("[Esc] close", theme::muted()),
        ])
    } else {
        Line::from(vec![
            Span::styled("[j/k] nav  ", theme::muted()),
            Span::styled("[n/N] new current/other  ", theme::title()),
            Span::styled("[Enter/e] edit  ", theme::key()),
            Span::styled("[Space] enable  ", Style::default().fg(theme::SUCCESS)),
            Span::styled("[x/Delete] delete  ", Style::default().fg(theme::DANGER)),
            Span::styled("[r] refresh  ", theme::muted()),
            Span::styled("[Esc] close", theme::muted()),
        ])
    };
    let path_line = match overlay.tab {
        HooksTab::User => format!("config: {}", overlay.user_config_path),
        HooksTab::Project => overlay
            .project_config_path
            .as_deref()
            .map(|path| format!("config: {path}"))
            .unwrap_or_else(|| "config: project .kairox/config.toml".to_string()),
        HooksTab::Templates => "templates: builtin hook starters".to_string(),
    };
    frame.render_widget(
        Paragraph::new(vec![
            hints,
            Line::from(Span::styled(path_line, theme::muted())),
        ]),
        chunks[2],
    );
}

fn render_tabs(overlay: &HooksOverlay) -> Line<'static> {
    Line::from(vec![
        tab_span(
            HooksTab::User,
            overlay.tab == HooksTab::User,
            overlay.user.len(),
        ),
        Span::raw("  "),
        tab_span(
            HooksTab::Project,
            overlay.tab == HooksTab::Project,
            overlay.project.len(),
        ),
        Span::raw("  "),
        tab_span(
            HooksTab::Templates,
            overlay.tab == HooksTab::Templates,
            overlay.templates.len(),
        ),
    ])
}

fn tab_span(tab: HooksTab, active: bool, count: usize) -> Span<'static> {
    let label = format!("{} ({count})", tab.label());
    if active {
        Span::styled(
            format!("[{label}]"),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!(" {label} "), theme::muted())
    }
}

fn render_hook_rows(
    area: Rect,
    frame: &mut Frame,
    hooks: &[HookSettingsView],
    mut state: ListState,
) {
    if hooks.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No hooks configured",
                theme::muted(),
            ))),
            area,
        );
        return;
    }

    let items = hooks
        .iter()
        .map(|hook| {
            let state_label = if hook.enabled { "enabled" } else { "disabled" };
            let matcher = hook
                .matcher
                .as_deref()
                .filter(|matcher| !matcher.is_empty())
                .unwrap_or("-");
            let timeout = hook
                .timeout_secs
                .map(|value| format!("{value}s"))
                .unwrap_or_else(|| "-".to_string());
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!("{:<8}", state_label),
                        Style::default().fg(if hook.enabled {
                            theme::SUCCESS
                        } else {
                            theme::MUTED
                        }),
                    ),
                    Span::styled(
                        format!(" {:<17}", hook.event),
                        Style::default().fg(theme::WARNING),
                    ),
                    Span::styled(
                        format!(" {}", hook.id),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        format!("  matcher: {matcher:<16} timeout: {timeout:<6} "),
                        theme::muted(),
                    ),
                    Span::raw(&hook.command),
                ]),
            ])
        })
        .collect::<Vec<_>>();
    let list = List::new(items).highlight_style(theme::selected());
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_template_rows(
    area: Rect,
    frame: &mut Frame,
    templates: &[HookTemplateView],
    mut state: ListState,
) {
    if templates.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No hook templates available",
                theme::muted(),
            ))),
            area,
        );
        return;
    }

    let items = templates
        .iter()
        .map(|template| {
            let matcher = template
                .matcher
                .as_deref()
                .filter(|matcher| !matcher.is_empty())
                .unwrap_or("-");
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!("{:<17}", template.event),
                        Style::default().fg(theme::WARNING),
                    ),
                    Span::styled(
                        format!(" {}", template.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!(" ({})", template.id), theme::muted()),
                ]),
                Line::from(vec![
                    Span::styled(format!("  matcher: {matcher:<16} "), theme::muted()),
                    Span::raw(&template.command),
                ]),
            ])
        })
        .collect::<Vec<_>>();
    let list = List::new(items).highlight_style(theme::selected());
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_hooks_editor(area: Rect, frame: &mut Frame, overlay: &HooksOverlay) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let rows = EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let selected = index == overlay.editor_field_index;
            let marker = if selected { "> " } else { "  " };
            let mut value = editor_field_value(&overlay.draft, *field);
            if *field == HookEditorField::Event {
                value.push_str("  (Left/Right cycles)");
            }
            let style = if selected {
                Style::default()
                    .fg(theme::WARNING)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::styled(marker, theme::title()),
                Span::styled(format!("{:<10}", editor_field_label(*field)), style),
                Span::raw(" "),
                Span::styled(value, theme::muted()),
            ])
        })
        .collect::<Vec<_>>();

    let body = Paragraph::new(rows)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::WARNING)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[0]);

    let hints = Line::from(vec![
        Span::styled("[Tab] field  ", theme::muted()),
        Span::styled("[Enter] save  ", theme::key()),
        Span::styled("[Del] clear  ", Style::default().fg(theme::DANGER)),
        Span::styled("[Esc] list", theme::muted()),
    ]);
    frame.render_widget(Paragraph::new(hints), chunks[1]);
}

fn scope_label(scope: ConfigScope) -> &'static str {
    match scope {
        ConfigScope::User => "user",
        ConfigScope::Project => "project",
        ConfigScope::Builtin => "builtin",
        ConfigScope::Local => "local",
    }
}

fn editor_field_label(field: HookEditorField) -> &'static str {
    match field {
        HookEditorField::Scope => "Scope",
        HookEditorField::Id => "Id",
        HookEditorField::Event => "Event",
        HookEditorField::Matcher => "Matcher",
        HookEditorField::Command => "Command",
        HookEditorField::StatusMessage => "Status",
        HookEditorField::TimeoutSecs => "Timeout",
        HookEditorField::Enabled => "Enabled",
    }
}

fn editor_field_value(draft: &HookDraft, field: HookEditorField) -> String {
    match field {
        HookEditorField::Scope => scope_label(draft.scope).to_string(),
        HookEditorField::Id => draft.id.clone(),
        HookEditorField::Event => draft.event.clone(),
        HookEditorField::Matcher => draft.matcher.clone(),
        HookEditorField::Command => draft.command.clone(),
        HookEditorField::StatusMessage => draft.status_message.clone(),
        HookEditorField::TimeoutSecs => draft.timeout_secs.clone(),
        HookEditorField::Enabled => draft.enabled.to_string(),
    }
}

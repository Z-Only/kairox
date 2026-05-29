//! Sessions panel rendering: list rows, archive manager modal, and contextual
//! action overlay. State and key handling live in [`super::state`].

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::{theme, ProjectInfo, SessionInfo};

use super::render_items::{
    action_key, action_label, archived_session_row_line, project_row_line, session_row_line,
};
use super::state::{
    archive_stats, archived_sessions, project_exists, session_list_rows, SelectedRow,
    SessionActionMode, SessionsPanel,
};

#[cfg(test)]
pub(super) use super::render_items::session_state_icon;

pub fn render_sessions(
    area: Rect,
    frame: &mut Frame,
    projects: &[ProjectInfo],
    sessions: &[SessionInfo],
    focused: bool,
    state: &mut ListState,
) {
    let border_style = theme::border(focused);

    let rows = session_list_rows(projects, sessions);
    let stats = archive_stats(sessions);
    if rows.is_empty() {
        state.select(None);
    } else if state
        .selected()
        .is_none_or(|selected| selected >= rows.len())
    {
        state.select(Some(0));
    }

    let items: Vec<ListItem> = rows
        .iter()
        .filter_map(|row| match row {
            super::state::SessionListRow::Project(project_id) => projects
                .iter()
                .find(|project| &project.id == project_id)
                .map(|project| ListItem::new(project_row_line(project, sessions))),
            super::state::SessionListRow::Session(session_id) => sessions
                .iter()
                .find(|session| &session.id == session_id)
                .map(|session| {
                    let nested = session
                        .project_id
                        .as_ref()
                        .is_some_and(|project_id| project_exists(projects, project_id));
                    ListItem::new(session_row_line(session, nested))
                }),
        })
        .collect();

    let title = if stats.total == 0 {
        " Projects / Sessions ".to_string()
    } else {
        format!(" Projects / Sessions · [A] Archive {} ", stats.total)
    };

    let list = List::new(items)
        .highlight_style(theme::selected())
        .highlight_symbol("> ")
        .block(
            Block::default()
                .borders(Borders::RIGHT)
                .title(Span::styled(title, theme::title()))
                .border_style(border_style),
        );
    frame.render_stateful_widget(list, area, state);
}

pub(super) fn render_archive_manager(
    area: Rect,
    frame: &mut Frame,
    panel: &SessionsPanel,
    projects: &[ProjectInfo],
    sessions: &[SessionInfo],
) {
    let modal_width = 72.min(area.width.saturating_sub(4));
    let modal_height = 16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Archive Manager ", theme::title()))
        .border_style(theme::border(true));
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    if inner.height < 3 {
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

    let stats = archive_stats(sessions);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Total: ", theme::muted()),
            Span::styled(
                stats.total.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Projects: ", theme::muted()),
            Span::styled(
                stats.projects.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ])),
        chunks[0],
    );

    let archived = archived_sessions(sessions);
    if archived.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No archived sessions",
                theme::muted(),
            ))),
            chunks[1],
        );
    } else {
        let items: Vec<ListItem> = archived
            .iter()
            .map(|session| ListItem::new(archived_session_row_line(session, projects)))
            .collect();
        let mut state = ListState::default();
        state.select(Some(panel.archive_cursor.min(archived.len() - 1)));
        let list = List::new(items)
            .highlight_style(theme::selected())
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, chunks[1], &mut state);
    }

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("[j/k] nav  ", theme::muted()),
            Span::styled("[Enter/r] restore  ", theme::key()),
            Span::styled("[d] delete  ", theme::key()),
            Span::styled("[Esc] close", theme::muted()),
        ])),
        chunks[2],
    );
}

pub(super) fn render_session_action_overlay(
    area: Rect,
    frame: &mut Frame,
    panel: &SessionsPanel,
    projects: &[ProjectInfo],
    sessions: &[SessionInfo],
) {
    let modal_width = 56.min(area.width.saturating_sub(4));
    let modal_height = 12.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Session Actions ", theme::title()))
        .border_style(theme::border(true));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let (label, title) = match panel.selected_target(projects, sessions) {
        Some(SelectedRow::Project(project)) => ("Project: ", project.display_name),
        Some(SelectedRow::Session(session)) => ("Session: ", session.title),
        None => ("Selection: ", "None".to_string()),
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(label, theme::muted()),
            Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
        ])),
        chunks[0],
    );

    match &panel.action_mode {
        SessionActionMode::Menu => {
            let actions = panel.available_actions(projects, sessions);
            if actions.is_empty() {
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "No actions available",
                        theme::muted(),
                    ))),
                    chunks[1],
                );
            } else {
                let items: Vec<ListItem> = actions
                    .iter()
                    .map(|action| {
                        ListItem::new(Line::from(vec![
                            Span::styled(format!("[{}] ", action_key(*action)), theme::key()),
                            Span::raw(action_label(*action)),
                        ]))
                    })
                    .collect();
                let mut state = ListState::default();
                state.select(Some(panel.action_cursor.min(actions.len() - 1)));
                let list = List::new(items).highlight_style(theme::selected());
                frame.render_stateful_widget(list, chunks[1], &mut state);
            }
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("[j/k] nav  ", theme::muted()),
                    Span::styled("[Enter] run  ", theme::key()),
                    Span::styled("[Esc] close", theme::muted()),
                ])),
                chunks[2],
            );
        }
        SessionActionMode::RenameSession { title, .. } => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Title: ", theme::key()),
                    Span::raw(title.clone()),
                    Span::styled("▌", theme::title()),
                ])),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("[Enter] save  ", theme::key()),
                    Span::styled("[Esc] cancel", theme::muted()),
                ])),
                chunks[2],
            );
        }
        SessionActionMode::RenameProject { display_name, .. } => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Project: ", theme::key()),
                    Span::raw(display_name.clone()),
                    Span::styled("▌", theme::title()),
                ])),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("[Enter] save  ", theme::key()),
                    Span::styled("[Esc] cancel", theme::muted()),
                ])),
                chunks[2],
            );
        }
        SessionActionMode::Worktree { branch_name, .. } => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Branch: ", theme::key()),
                    Span::raw(branch_name.clone()),
                    Span::styled("▌", theme::title()),
                ])),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("[Enter] create  ", theme::key()),
                    Span::styled("[Esc] cancel", theme::muted()),
                ])),
                chunks[2],
            );
        }
    }
}

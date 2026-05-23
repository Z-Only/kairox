//! Sessions panel rendering: list rows, archive manager modal, and contextual
//! action overlay. State and key handling live in [`super::state`].

use agent_core::{
    ProjectGitStatus, ProjectGitStatusKind, ProjectInstructionSummary, ProjectSessionVisibility,
};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::{ProjectInfo, SessionInfo, SessionState};

use super::state::{
    archive_stats, archived_sessions, project_exists, session_list_rows, SelectedRow,
    SessionAction, SessionActionMode, SessionListRow, SessionsPanel,
};

pub(super) fn session_state_icon(state: &SessionState) -> (&'static str, Color) {
    match state {
        SessionState::Active => ("●", Color::Green),
        SessionState::Idle => ("○", Color::DarkGray),
        SessionState::Error(_) => ("✕", Color::Red),
        SessionState::AwaitingPermission => ("⚠", Color::Yellow),
    }
}

pub fn render_sessions(
    area: Rect,
    frame: &mut Frame,
    projects: &[ProjectInfo],
    sessions: &[SessionInfo],
    focused: bool,
    state: &mut ListState,
) {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

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
            SessionListRow::Project(project_id) => projects
                .iter()
                .find(|project| &project.id == project_id)
                .map(|project| ListItem::new(project_row_line(project, sessions))),
            SessionListRow::Session(session_id) => sessions
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
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("❯ ")
        .block(
            Block::default()
                .borders(Borders::RIGHT)
                .title(title)
                .border_style(border_style),
        );
    frame.render_stateful_widget(list, area, state);
}

fn project_row_line(project: &ProjectInfo, sessions: &[SessionInfo]) -> Line<'static> {
    let session_count = sessions
        .iter()
        .filter(|session| !session.archived && session.project_id.as_ref() == Some(&project.id))
        .count();
    let expanded = if project.expanded { "▾" } else { "▸" };
    let mut spans = vec![
        Span::styled(format!("{expanded} "), Style::default().fg(Color::Cyan)),
        Span::styled(
            project.display_name.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" ({session_count})"),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    if let Some(status) = &project.git_status {
        let (label, color) = project_git_status_label(status);
        spans.push(Span::styled(
            format!(" · {label}"),
            Style::default().fg(color),
        ));
    }

    if let Some(summary) = &project.instruction_summary {
        spans.push(Span::styled(
            format!(" · {}", project_instruction_label(summary)),
            Style::default().fg(Color::DarkGray),
        ));
    }

    Line::from(spans)
}

fn archive_project_label(projects: &[ProjectInfo], session: &SessionInfo) -> Option<String> {
    let project_id = session.project_id.as_ref()?;
    Some(
        projects
            .iter()
            .find(|project| &project.id == project_id)
            .map(|project| project.display_name.clone())
            .unwrap_or_else(|| project_id.to_string()),
    )
}

fn archived_session_row_line(session: &SessionInfo, projects: &[ProjectInfo]) -> Line<'static> {
    let mut spans = vec![
        Span::styled("○ ", Style::default().fg(Color::DarkGray)),
        Span::raw(session.title.clone()),
        Span::styled(
            format!(" [{}]", session.model_profile),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ];

    let mut metadata = Vec::new();
    if let Some(project) = archive_project_label(projects, session) {
        metadata.push(project);
    }
    if let Some(branch) = session
        .branch
        .as_deref()
        .filter(|branch| !branch.is_empty())
    {
        metadata.push(format!("branch {branch}"));
    }
    if let Some(path) = session
        .worktree_path
        .as_deref()
        .filter(|path| !path.is_empty())
        .map(compact_worktree_path)
    {
        metadata.push(path);
    }
    if !metadata.is_empty() {
        spans.push(Span::styled(
            format!(" · {}", metadata.join(" · ")),
            Style::default().fg(Color::DarkGray),
        ));
    }

    Line::from(spans)
}

fn session_row_line(session: &SessionInfo, nested: bool) -> Line<'static> {
    let (icon, icon_color) = session_state_icon(&session.state);
    let pin = if session.pinned { "📌 " } else { "" };
    let archived = if session.archived { " [archived]" } else { "" };
    let metadata = session_metadata_label(session);
    let prefix = if nested { "  " } else { "" };
    let mut spans = vec![
        Span::raw(prefix.to_string()),
        Span::styled(format!("{pin}{icon} "), Style::default().fg(icon_color)),
        Span::raw(session.title.clone()),
        Span::styled(
            format!(" [{}]{archived}", session.model_profile),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ];
    if let Some(metadata) = metadata {
        spans.push(Span::styled(
            format!(" {metadata}"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    if let SessionState::Error(e) = &session.state {
        spans.push(Span::styled(
            format!(" {e}"),
            Style::default().fg(Color::Red),
        ));
    }
    Line::from(spans)
}

fn project_git_status_label(status: &ProjectGitStatus) -> (String, Color) {
    match status.kind {
        ProjectGitStatusKind::NotInitialized => ("git not initialized".into(), Color::Yellow),
        ProjectGitStatusKind::Clean => (
            status
                .branch
                .as_deref()
                .map(|branch| format!("git {branch}"))
                .unwrap_or_else(|| "git clean".into()),
            Color::Green,
        ),
        ProjectGitStatusKind::Dirty => (
            status
                .branch
                .as_deref()
                .map(|branch| format!("dirty {branch}"))
                .unwrap_or_else(|| "dirty".into()),
            Color::Yellow,
        ),
        ProjectGitStatusKind::Detached => ("detached".into(), Color::Yellow),
        ProjectGitStatusKind::MissingPath => ("missing path".into(), Color::Red),
        ProjectGitStatusKind::Error => (
            status
                .message
                .as_deref()
                .filter(|message| !message.is_empty())
                .map(|message| format!("git error: {message}"))
                .unwrap_or_else(|| "git error".into()),
            Color::Red,
        ),
    }
}

fn project_instruction_label(summary: &ProjectInstructionSummary) -> String {
    if summary.source_paths.is_empty() {
        return "instructions none".into();
    }
    let names = summary
        .source_paths
        .iter()
        .filter_map(|path| std::path::Path::new(path).file_name())
        .filter_map(std::ffi::OsStr::to_str)
        .take(2)
        .collect::<Vec<_>>();
    if names.is_empty() {
        format!("instructions {}", summary.source_paths.len())
    } else {
        format!("instructions {}", names.join(", "))
    }
}

fn session_metadata_label(session: &SessionInfo) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(visibility) = &session.visibility {
        if matches!(visibility, ProjectSessionVisibility::DraftHidden) {
            parts.push("draft".to_string());
        }
    }
    if let Some(branch) = session
        .branch
        .as_deref()
        .filter(|branch| !branch.is_empty())
    {
        parts.push(format!("branch {branch}"));
    }
    if let Some(path) = session
        .worktree_path
        .as_deref()
        .filter(|path| !path.is_empty())
        .map(compact_worktree_path)
    {
        parts.push(path);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

fn compact_worktree_path(path: &str) -> String {
    path.split_once(".kairox/")
        .map(|(_, suffix)| suffix.to_string())
        .unwrap_or_else(|| path.to_string())
}

fn action_label(action: SessionAction) -> &'static str {
    match action {
        SessionAction::Rename => "Rename",
        SessionAction::Archive => "Archive",
        SessionAction::Restore => "Restore",
        SessionAction::Delete => "Delete permanently",
        SessionAction::RemoveProject => "Remove project",
        SessionAction::MoveProjectUp => "Move project up",
        SessionAction::MoveProjectDown => "Move project down",
        SessionAction::ToggleExpanded => "Expand/collapse",
        SessionAction::NewDraft => "New draft session",
        SessionAction::NewWorktree => "New worktree session",
        SessionAction::GitStatus => "Refresh git status",
        SessionAction::InitGit => "Initialize git",
        SessionAction::Instructions => "Show instructions",
    }
}

fn action_key(action: SessionAction) -> &'static str {
    match action {
        SessionAction::Rename => "r",
        SessionAction::Archive => "a",
        SessionAction::Restore => "r",
        SessionAction::Delete => "d",
        SessionAction::RemoveProject => "d",
        SessionAction::MoveProjectUp => "↑",
        SessionAction::MoveProjectDown => "↓",
        SessionAction::ToggleExpanded => "e",
        SessionAction::NewDraft => "n",
        SessionAction::NewWorktree => "w",
        SessionAction::GitStatus => "g",
        SessionAction::InitGit => "i",
        SessionAction::Instructions => "I",
    }
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
        .title(Span::styled(
            " Archive Manager ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));
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
            Span::styled("Total: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                stats.total.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Projects: ", Style::default().fg(Color::DarkGray)),
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
                Style::default().fg(Color::DarkGray),
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
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("❯ ");
        frame.render_stateful_widget(list, chunks[1], &mut state);
    }

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Enter/r] restore  ", Style::default().fg(Color::Yellow)),
            Span::styled("[d] delete  ", Style::default().fg(Color::Yellow)),
            Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
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
        .title(Span::styled(
            " Session Actions ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

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
            Span::styled(label, Style::default().fg(Color::DarkGray)),
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
                        Style::default().fg(Color::DarkGray),
                    ))),
                    chunks[1],
                );
            } else {
                let items: Vec<ListItem> = actions
                    .iter()
                    .map(|action| {
                        ListItem::new(Line::from(vec![
                            Span::styled(
                                format!("[{}] ", action_key(*action)),
                                Style::default().fg(Color::Yellow),
                            ),
                            Span::raw(action_label(*action)),
                        ]))
                    })
                    .collect();
                let mut state = ListState::default();
                state.select(Some(panel.action_cursor.min(actions.len() - 1)));
                let list = List::new(items).highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                );
                frame.render_stateful_widget(list, chunks[1], &mut state);
            }
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
                    Span::styled("[Enter] run  ", Style::default().fg(Color::Yellow)),
                    Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
                ])),
                chunks[2],
            );
        }
        SessionActionMode::RenameSession { title, .. } => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Title: ", Style::default().fg(Color::Yellow)),
                    Span::raw(title.clone()),
                    Span::styled("▌", Style::default().fg(Color::Cyan)),
                ])),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
                    Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
                ])),
                chunks[2],
            );
        }
        SessionActionMode::RenameProject { display_name, .. } => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Project: ", Style::default().fg(Color::Yellow)),
                    Span::raw(display_name.clone()),
                    Span::styled("▌", Style::default().fg(Color::Cyan)),
                ])),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
                    Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
                ])),
                chunks[2],
            );
        }
        SessionActionMode::Worktree { branch_name, .. } => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Branch: ", Style::default().fg(Color::Yellow)),
                    Span::raw(branch_name.clone()),
                    Span::styled("▌", Style::default().fg(Color::Cyan)),
                ])),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("[Enter] create  ", Style::default().fg(Color::Yellow)),
                    Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
                ])),
                chunks[2],
            );
        }
    }
}

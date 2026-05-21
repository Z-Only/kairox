use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, ProjectInfo, SessionInfo, SessionState,
};
use agent_core::{ProjectId, ProjectSessionVisibility, SessionId};
use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAction {
    Rename,
    Archive,
    Restore,
    Delete,
    NewDraft,
    NewWorktree,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionActionMode {
    Menu,
    Rename {
        session_id: SessionId,
        title: String,
    },
    Worktree {
        project_id: ProjectId,
        branch_name: String,
    },
}

#[allow(dead_code)]
pub struct SessionsPanel {
    focused: bool,
    pub state: ListState,
    pub context_menu_open: bool,
    pub search_query: Option<String>,
    action_cursor: usize,
    action_mode: SessionActionMode,
}

impl Default for SessionsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionsPanel {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            focused: false,
            state,
            context_menu_open: false,
            search_query: None,
            action_cursor: 0,
            action_mode: SessionActionMode::Menu,
        }
    }

    pub fn selected_session<'a>(&self, sessions: &'a [SessionInfo]) -> Option<&'a SessionInfo> {
        self.state.selected().and_then(|i| sessions.get(i))
    }

    #[allow(dead_code)]
    pub fn selected_session_id(&self, sessions: &[SessionInfo]) -> Option<SessionId> {
        self.selected_session(sessions).map(|s| s.id.clone())
    }

    #[allow(dead_code)]
    pub fn filtered_sessions<'a>(&self, sessions: &'a [SessionInfo]) -> Vec<&'a SessionInfo> {
        if let Some(query) = &self.search_query {
            let q = query.to_lowercase();
            sessions
                .iter()
                .filter(|s| {
                    s.title.to_lowercase().contains(&q)
                        || s.model_profile.to_lowercase().contains(&q)
                })
                .collect()
        } else {
            sessions.iter().collect()
        }
    }

    pub fn scroll_up(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.state.selected().unwrap_or(0);
        let next = if i == 0 { len - 1 } else { i - 1 };
        self.state.select(Some(next));
    }

    pub fn scroll_down(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let i = self.state.selected().unwrap_or(0);
        let next = if i >= len - 1 { 0 } else { i + 1 };
        self.state.select(Some(next));
    }

    pub fn open_action_menu(&mut self, sessions: &[SessionInfo]) -> bool {
        if self.selected_session(sessions).is_none() {
            self.close_action_menu();
            return false;
        }
        self.context_menu_open = true;
        self.action_cursor = 0;
        self.action_mode = SessionActionMode::Menu;
        true
    }

    pub fn start_rename_for_selected(&mut self, sessions: &[SessionInfo]) -> bool {
        let Some(session) = self.selected_session(sessions) else {
            self.close_action_menu();
            return false;
        };
        if session.archived {
            return false;
        }
        self.context_menu_open = true;
        self.action_cursor = 0;
        self.action_mode = SessionActionMode::Rename {
            session_id: session.id.clone(),
            title: session.title.clone(),
        };
        true
    }

    pub fn close_action_menu(&mut self) {
        self.context_menu_open = false;
        self.action_cursor = 0;
        self.action_mode = SessionActionMode::Menu;
    }

    fn available_actions_for(session: &SessionInfo) -> &'static [SessionAction] {
        if session.archived {
            &[SessionAction::Restore, SessionAction::Delete]
        } else if session.project_id.is_some() {
            &[
                SessionAction::Rename,
                SessionAction::Archive,
                SessionAction::NewDraft,
                SessionAction::NewWorktree,
            ]
        } else {
            &[SessionAction::Rename, SessionAction::Archive]
        }
    }

    fn available_actions(&self, sessions: &[SessionInfo]) -> &'static [SessionAction] {
        self.selected_session(sessions)
            .map(Self::available_actions_for)
            .unwrap_or(&[])
    }

    fn current_action(&self, sessions: &[SessionInfo]) -> Option<SessionAction> {
        self.available_actions(sessions)
            .get(self.action_cursor)
            .copied()
    }

    fn move_action_down(&mut self, sessions: &[SessionInfo]) {
        let len = self.available_actions(sessions).len();
        if len == 0 {
            self.action_cursor = 0;
            return;
        }
        self.action_cursor = (self.action_cursor + 1).min(len - 1);
    }

    fn move_action_up(&mut self) {
        self.action_cursor = self.action_cursor.saturating_sub(1);
    }

    fn activate_action(&mut self, action: SessionAction, session: &SessionInfo) -> Vec<Command> {
        match action {
            SessionAction::Rename => {
                if !session.archived {
                    self.action_mode = SessionActionMode::Rename {
                        session_id: session.id.clone(),
                        title: session.title.clone(),
                    };
                }
                Vec::new()
            }
            SessionAction::Archive => {
                self.close_action_menu();
                vec![Command::ArchiveSession {
                    session_id: session.id.clone(),
                }]
            }
            SessionAction::Restore => {
                self.close_action_menu();
                vec![Command::RestoreSession {
                    session_id: session.id.clone(),
                }]
            }
            SessionAction::Delete => {
                self.close_action_menu();
                vec![Command::DeleteSession {
                    session_id: session.id.clone(),
                }]
            }
            SessionAction::NewDraft => {
                self.close_action_menu();
                session
                    .project_id
                    .clone()
                    .map(|project_id| Command::CreateProjectDraftSession { project_id })
                    .into_iter()
                    .collect()
            }
            SessionAction::NewWorktree => {
                if let Some(project_id) = session.project_id.clone() {
                    self.action_mode = SessionActionMode::Worktree {
                        project_id,
                        branch_name: String::new(),
                    };
                }
                Vec::new()
            }
        }
    }

    fn handle_menu_key(&mut self, ctx: &EventContext, code: KeyCode) -> Vec<Command> {
        match code {
            KeyCode::Esc => {
                self.close_action_menu();
                Vec::new()
            }
            KeyCode::Char('x') => {
                self.open_action_menu(ctx.sessions);
                Vec::new()
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_action_down(ctx.sessions);
                Vec::new()
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_action_up();
                Vec::new()
            }
            KeyCode::Enter => {
                let Some(session) = self.selected_session(ctx.sessions).cloned() else {
                    self.close_action_menu();
                    return Vec::new();
                };
                let Some(action) = self.current_action(ctx.sessions) else {
                    return Vec::new();
                };
                self.activate_action(action, &session)
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                let Some(session) = self.selected_session(ctx.sessions).cloned() else {
                    self.close_action_menu();
                    return Vec::new();
                };
                let action = if session.archived {
                    SessionAction::Restore
                } else {
                    SessionAction::Rename
                };
                self.activate_action(action, &session)
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                let Some(session) = self.selected_session(ctx.sessions).cloned() else {
                    self.close_action_menu();
                    return Vec::new();
                };
                if session.archived {
                    Vec::new()
                } else {
                    self.activate_action(SessionAction::Archive, &session)
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                let Some(session) = self.selected_session(ctx.sessions).cloned() else {
                    self.close_action_menu();
                    return Vec::new();
                };
                if session.archived {
                    self.activate_action(SessionAction::Delete, &session)
                } else {
                    Vec::new()
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let Some(session) = self.selected_session(ctx.sessions).cloned() else {
                    self.close_action_menu();
                    return Vec::new();
                };
                if session.project_id.is_some() && !session.archived {
                    self.activate_action(SessionAction::NewDraft, &session)
                } else {
                    Vec::new()
                }
            }
            KeyCode::Char('w') | KeyCode::Char('W') => {
                let Some(session) = self.selected_session(ctx.sessions).cloned() else {
                    self.close_action_menu();
                    return Vec::new();
                };
                if session.project_id.is_some() && !session.archived {
                    self.activate_action(SessionAction::NewWorktree, &session)
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    fn handle_rename_key(&mut self, code: KeyCode) -> Vec<Command> {
        match code {
            KeyCode::Esc => {
                self.close_action_menu();
                Vec::new()
            }
            KeyCode::Enter => {
                let command = match &self.action_mode {
                    SessionActionMode::Rename { session_id, title } => {
                        let title = title.trim().to_string();
                        if title.is_empty() {
                            None
                        } else {
                            Some(Command::RenameSession {
                                session_id: session_id.clone(),
                                title,
                            })
                        }
                    }
                    SessionActionMode::Menu | SessionActionMode::Worktree { .. } => None,
                };
                self.close_action_menu();
                command.into_iter().collect()
            }
            KeyCode::Backspace => {
                if let SessionActionMode::Rename { title, .. } = &mut self.action_mode {
                    title.pop();
                }
                Vec::new()
            }
            KeyCode::Char(c) => {
                if let SessionActionMode::Rename { title, .. } = &mut self.action_mode {
                    title.push(c);
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn handle_worktree_key(&mut self, code: KeyCode) -> Vec<Command> {
        match code {
            KeyCode::Esc => {
                self.close_action_menu();
                Vec::new()
            }
            KeyCode::Enter => {
                let command = match &self.action_mode {
                    SessionActionMode::Worktree {
                        project_id,
                        branch_name,
                    } => {
                        let branch_name = branch_name.trim().to_string();
                        if branch_name.is_empty() {
                            None
                        } else {
                            Some(Command::CreateProjectWorktreeSession {
                                project_id: project_id.clone(),
                                branch_name,
                            })
                        }
                    }
                    SessionActionMode::Menu | SessionActionMode::Rename { .. } => None,
                };
                self.close_action_menu();
                command.into_iter().collect()
            }
            KeyCode::Backspace => {
                if let SessionActionMode::Worktree { branch_name, .. } = &mut self.action_mode {
                    branch_name.pop();
                }
                Vec::new()
            }
            KeyCode::Char(c) => {
                if let SessionActionMode::Worktree { branch_name, .. } = &mut self.action_mode {
                    branch_name.push(c);
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    pub fn render_action_overlay(&self, area: Rect, frame: &mut Frame, sessions: &[SessionInfo]) {
        if !self.context_menu_open {
            return;
        }
        render_session_action_overlay(area, frame, self, sessions);
    }
}

fn session_state_icon(state: &SessionState) -> (&'static str, Color) {
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

    let (project_area, list_area) = if projects.is_empty() {
        (None, area)
    } else {
        let project_height = (projects.len() as u16 + 2).min(area.height.saturating_sub(3).max(1));
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(project_height), Constraint::Min(1)])
            .split(area);
        (Some(chunks[0]), chunks[1])
    };

    if let Some(project_area) = project_area {
        render_project_summary(project_area, frame, projects, sessions, border_style);
    }

    let items: Vec<ListItem> = sessions
        .iter()
        .map(|session| {
            let (icon, icon_color) = session_state_icon(&session.state);
            let pin = if session.pinned { "📌 " } else { "" };
            let archived = if session.archived { " [archived]" } else { "" };
            let metadata = session_metadata_label(session);
            let mut spans = vec![
                Span::styled(format!("{pin}{icon} "), Style::default().fg(icon_color)),
                Span::raw(&session.title),
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
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("❯ ")
        .block(
            Block::default()
                .borders(Borders::RIGHT)
                .title(" Sessions ")
                .border_style(border_style),
        );
    frame.render_stateful_widget(list, list_area, state);
}

fn render_project_summary(
    area: Rect,
    frame: &mut Frame,
    projects: &[ProjectInfo],
    sessions: &[SessionInfo],
    border_style: Style,
) {
    let lines = projects.iter().map(|project| {
        let session_count = sessions
            .iter()
            .filter(|session| session.project_id.as_ref() == Some(&project.id))
            .count();
        let expanded = if project.expanded { "▾" } else { "▸" };
        Line::from(vec![
            Span::styled(format!("{expanded} "), Style::default().fg(Color::Cyan)),
            Span::raw(project.display_name.clone()),
            Span::styled(
                format!(" ({session_count})"),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    });
    let paragraph = Paragraph::new(lines.collect::<Vec<_>>()).block(
        Block::default()
            .borders(Borders::RIGHT | Borders::BOTTOM)
            .title(" Projects ")
            .border_style(border_style),
    );
    frame.render_widget(paragraph, area);
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
        SessionAction::NewDraft => "New draft session",
        SessionAction::NewWorktree => "New worktree session",
    }
}

fn action_key(action: SessionAction) -> &'static str {
    match action {
        SessionAction::Rename => "r",
        SessionAction::Archive => "a",
        SessionAction::Restore => "r",
        SessionAction::Delete => "d",
        SessionAction::NewDraft => "n",
        SessionAction::NewWorktree => "w",
    }
}

fn render_session_action_overlay(
    area: Rect,
    frame: &mut Frame,
    panel: &SessionsPanel,
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

    let title = panel
        .selected_session(sessions)
        .map(|s| s.title.as_str())
        .unwrap_or("No session selected");
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Session: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                title.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ])),
        chunks[0],
    );

    match &panel.action_mode {
        SessionActionMode::Menu => {
            let actions = panel.available_actions(sessions);
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
        SessionActionMode::Rename { title, .. } => {
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

impl Component for SessionsPanel {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };

        if !self.context_menu_open {
            if matches!(key.code, KeyCode::Char('x')) {
                self.open_action_menu(ctx.sessions);
            }
            return (Vec::new(), Vec::new());
        }

        let commands = match self.action_mode {
            SessionActionMode::Menu => self.handle_menu_key(ctx, key.code),
            SessionActionMode::Rename { .. } => self.handle_rename_key(key.code),
            SessionActionMode::Worktree { .. } => self.handle_worktree_key(key.code),
        };
        (Vec::new(), commands)
    }

    fn handle_effect(&mut self, _effect: &CrossPanelEffect) {}

    fn render(&self, area: Rect, frame: &mut Frame) {
        let _ = (area, frame);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{ProjectId, ProjectSessionVisibility};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn make_session(title: &str, state: SessionState, pinned: bool) -> SessionInfo {
        SessionInfo {
            id: SessionId::new(),
            title: title.into(),
            model_profile: "fast".into(),
            state,
            pinned,
            archived: false,
            project_id: None,
            worktree_path: None,
            branch: None,
            visibility: None,
        }
    }

    fn make_archived_session(title: &str) -> SessionInfo {
        SessionInfo {
            archived: true,
            ..make_session(title, SessionState::Idle, false)
        }
    }

    fn make_project(title: &str) -> ProjectInfo {
        ProjectInfo {
            id: ProjectId::from_string(format!("prj_{title}")),
            display_name: title.into(),
            root_path: format!("/tmp/{title}"),
            expanded: true,
        }
    }

    fn make_project_session(
        title: &str,
        project_id: ProjectId,
        branch: Option<&str>,
        worktree_path: Option<&str>,
    ) -> SessionInfo {
        SessionInfo {
            project_id: Some(project_id),
            branch: branch.map(str::to_string),
            worktree_path: worktree_path.map(str::to_string),
            visibility: Some(ProjectSessionVisibility::Visible),
            ..make_session(title, SessionState::Idle, false)
        }
    }

    fn ctx<'a>(
        sessions: &'a [SessionInfo],
        current_session_id: &'a Option<SessionId>,
        workspace_id: &'a agent_core::WorkspaceId,
        projection: &'a agent_core::projection::SessionProjection,
    ) -> EventContext<'a> {
        EventContext {
            focus: crate::components::FocusTarget::Sessions,
            current_session: projection,
            sessions,
            model_profile: "fast",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: false,
            workspace_id,
            current_session_id,
        }
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn filtered_sessions_returns_all_when_no_query() {
        let panel = SessionsPanel::new();
        let sessions = vec![
            make_session("main", SessionState::Active, false),
            make_session("debug", SessionState::Idle, false),
        ];
        let filtered = panel.filtered_sessions(&sessions);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filtered_sessions_matches_title_case_insensitive() {
        let mut panel = SessionsPanel::new();
        panel.search_query = Some("MAIN".into());
        let sessions = vec![
            make_session("main session", SessionState::Active, false),
            make_session("debug", SessionState::Idle, false),
        ];
        let filtered = panel.filtered_sessions(&sessions);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "main session");
    }

    #[test]
    fn selected_session_id_returns_correct_id() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![
            make_session("first", SessionState::Active, false),
            make_session("second", SessionState::Idle, false),
        ];
        panel.state.select(Some(1));
        assert_eq!(
            panel.selected_session_id(&sessions),
            Some(sessions[1].id.clone())
        );
    }

    #[test]
    fn session_state_icon_values() {
        assert_eq!(session_state_icon(&SessionState::Active).0, "●");
        assert_eq!(session_state_icon(&SessionState::Idle).0, "○");
        assert_eq!(
            session_state_icon(&SessionState::Error("err".into())).0,
            "✕"
        );
        assert_eq!(session_state_icon(&SessionState::AwaitingPermission).0, "⚠");
    }

    #[test]
    fn context_menu_key_opens_session_actions_for_selection() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![
            make_session("main", SessionState::Active, false),
            make_session("debug", SessionState::Idle, false),
        ];
        panel.state.select(Some(1));
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();
        let current = Some(sessions[0].id.clone());

        let (effects, commands) = panel.handle_event(
            &ctx(&sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('x')),
        );

        assert!(effects.is_empty());
        assert!(commands.is_empty());
        assert!(panel.context_menu_open);
    }

    #[test]
    fn action_overlay_emits_archive_for_visible_session() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![
            make_session("main", SessionState::Active, false),
            make_session("debug", SessionState::Idle, false),
        ];
        panel.state.select(Some(1));
        panel.context_menu_open = true;
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();
        let current = Some(sessions[0].id.clone());

        let (_, commands) = panel.handle_event(
            &ctx(&sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('a')),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::ArchiveSession { session_id }] if session_id == &sessions[1].id
        ));
        assert!(!panel.context_menu_open);
    }

    #[test]
    fn action_overlay_emits_restore_for_archived_session() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![
            make_session("main", SessionState::Active, false),
            make_archived_session("old"),
        ];
        panel.state.select(Some(1));
        panel.context_menu_open = true;
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();
        let current = Some(sessions[0].id.clone());

        let (_, commands) = panel.handle_event(
            &ctx(&sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('r')),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::RestoreSession { session_id }] if session_id == &sessions[1].id
        ));
        assert!(!panel.context_menu_open);
    }

    #[test]
    fn action_overlay_emits_delete_for_archived_session() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![make_archived_session("old")];
        panel.context_menu_open = true;
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();
        let current = None;

        let (_, commands) = panel.handle_event(
            &ctx(&sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('d')),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::DeleteSession { session_id }] if session_id == &sessions[0].id
        ));
        assert!(!panel.context_menu_open);
    }

    #[test]
    fn rename_inline_mode_emits_rename_command() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![make_session("main", SessionState::Active, false)];
        panel.context_menu_open = true;
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();
        let current = Some(sessions[0].id.clone());

        let (_, commands) = panel.handle_event(
            &ctx(&sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('r')),
        );
        assert!(commands.is_empty());
        let (_, commands) = panel.handle_event(
            &ctx(&sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('!')),
        );
        assert!(commands.is_empty());
        let (_, commands) = panel.handle_event(
            &ctx(&sessions, &current, &workspace, &projection),
            &key(KeyCode::Enter),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::RenameSession { session_id, title }]
                if session_id == &sessions[0].id && title == "main!"
        ));
        assert!(!panel.context_menu_open);
    }

    #[test]
    fn render_sessions_shows_projects_and_branch_worktree_metadata() {
        let project = make_project("alpha");
        let sessions = vec![make_project_session(
            "Worktree session",
            project.id.clone(),
            Some("feat/tui"),
            Some("/tmp/alpha/.kairox/worktrees/feat-tui"),
        )];
        let mut panel_state = ListState::default();

        let backend = ratatui::backend::TestBackend::new(72, 8);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                render_sessions(
                    frame.area(),
                    frame,
                    &[project],
                    &sessions,
                    true,
                    &mut panel_state,
                );
            })
            .expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");

        assert!(text.contains("Projects"));
        assert!(text.contains("alpha"));
        assert!(text.contains("Worktree session"));
        assert!(text.contains("feat/tui"));
        assert!(text.contains("worktrees/feat-tui"));
    }

    #[test]
    fn action_overlay_emits_create_project_draft_for_project_session() {
        let project = make_project("alpha");
        let sessions = vec![make_project_session(
            "alpha session",
            project.id.clone(),
            Some("main"),
            Some("/tmp/alpha"),
        )];
        let mut panel = SessionsPanel::new();
        panel.context_menu_open = true;
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();
        let current = Some(sessions[0].id.clone());

        let (_, commands) = panel.handle_event(
            &ctx(&sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('n')),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::CreateProjectDraftSession { project_id }]
                if project_id == &project.id
        ));
        assert!(!panel.context_menu_open);
    }

    #[test]
    fn worktree_input_emits_create_worktree_session_with_branch() {
        let project = make_project("alpha");
        let sessions = vec![make_project_session(
            "alpha session",
            project.id.clone(),
            Some("main"),
            Some("/tmp/alpha"),
        )];
        let mut panel = SessionsPanel::new();
        panel.context_menu_open = true;
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();
        let current = Some(sessions[0].id.clone());

        let (_, commands) = panel.handle_event(
            &ctx(&sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('w')),
        );
        assert!(commands.is_empty());

        for ch in "feat/tui-parity".chars() {
            let (_, commands) = panel.handle_event(
                &ctx(&sessions, &current, &workspace, &projection),
                &key(KeyCode::Char(ch)),
            );
            assert!(commands.is_empty());
        }

        let (_, commands) = panel.handle_event(
            &ctx(&sessions, &current, &workspace, &projection),
            &key(KeyCode::Enter),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::CreateProjectWorktreeSession { project_id, branch_name }]
                if project_id == &project.id && branch_name == "feat/tui-parity"
        ));
        assert!(!panel.context_menu_open);
    }
}

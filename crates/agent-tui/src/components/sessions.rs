use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, ProjectInfo, SessionInfo, SessionState,
};
use agent_core::{
    ProjectGitStatus, ProjectGitStatusKind, ProjectId, ProjectInstructionSummary,
    ProjectSessionVisibility, SessionId,
};
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
    RemoveProject,
    MoveProjectUp,
    MoveProjectDown,
    ToggleExpanded,
    NewDraft,
    NewWorktree,
    GitStatus,
    InitGit,
    Instructions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionActionMode {
    Menu,
    RenameSession {
        session_id: SessionId,
        title: String,
    },
    RenameProject {
        project_id: ProjectId,
        display_name: String,
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
    pub archive_manager_open: bool,
    pub search_query: Option<String>,
    action_cursor: usize,
    archive_cursor: usize,
    action_mode: SessionActionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveStats {
    pub total: usize,
    pub projects: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionListRow {
    Project(ProjectId),
    Session(SessionId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SelectedRow {
    Project(ProjectInfo),
    Session(SessionInfo),
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
            archive_manager_open: false,
            search_query: None,
            action_cursor: 0,
            archive_cursor: 0,
            action_mode: SessionActionMode::Menu,
        }
    }

    pub fn selected_session<'a>(&self, sessions: &'a [SessionInfo]) -> Option<&'a SessionInfo> {
        self.state.selected().and_then(|i| sessions.get(i))
    }

    pub fn selected_session_in<'a>(
        &self,
        projects: &[ProjectInfo],
        sessions: &'a [SessionInfo],
    ) -> Option<&'a SessionInfo> {
        match self.selected_row(projects, sessions)? {
            SessionListRow::Session(session_id) => {
                sessions.iter().find(|session| session.id == session_id)
            }
            SessionListRow::Project(_) => None,
        }
    }

    pub fn selected_row(
        &self,
        projects: &[ProjectInfo],
        sessions: &[SessionInfo],
    ) -> Option<SessionListRow> {
        self.state
            .selected()
            .and_then(|index| session_list_rows(projects, sessions).get(index).cloned())
    }

    fn selected_target(
        &self,
        projects: &[ProjectInfo],
        sessions: &[SessionInfo],
    ) -> Option<SelectedRow> {
        match self.selected_row(projects, sessions)? {
            SessionListRow::Project(project_id) => projects
                .iter()
                .find(|project| project.id == project_id)
                .cloned()
                .map(SelectedRow::Project),
            SessionListRow::Session(session_id) => sessions
                .iter()
                .find(|session| session.id == session_id)
                .cloned()
                .map(SelectedRow::Session),
        }
    }

    #[allow(dead_code)]
    pub fn selected_session_id(&self, sessions: &[SessionInfo]) -> Option<SessionId> {
        self.selected_session(sessions).map(|s| s.id.clone())
    }

    #[allow(dead_code)]
    pub fn filtered_sessions<'a>(&self, sessions: &'a [SessionInfo]) -> Vec<&'a SessionInfo> {
        let visible_sessions = sessions.iter().filter(|session| !session.archived);
        if let Some(query) = &self.search_query {
            let q = query.to_lowercase();
            visible_sessions
                .filter(|s| {
                    s.title.to_lowercase().contains(&q)
                        || s.model_profile.to_lowercase().contains(&q)
                })
                .collect()
        } else {
            visible_sessions.collect()
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

    pub fn open_action_menu(&mut self, projects: &[ProjectInfo], sessions: &[SessionInfo]) -> bool {
        if self.selected_target(projects, sessions).is_none() {
            self.close_action_menu();
            return false;
        }
        self.close_archive_manager();
        self.context_menu_open = true;
        self.action_cursor = 0;
        self.action_mode = SessionActionMode::Menu;
        true
    }

    pub fn open_archive_manager(&mut self, sessions: &[SessionInfo]) -> bool {
        self.close_action_menu();
        self.archive_manager_open = true;
        self.archive_cursor = self
            .archive_cursor
            .min(archive_stats(sessions).total.saturating_sub(1));
        true
    }

    pub fn close_archive_manager(&mut self) {
        self.archive_manager_open = false;
        self.archive_cursor = 0;
    }

    pub fn is_overlay_open(&self) -> bool {
        self.context_menu_open || self.archive_manager_open
    }

    pub fn start_rename_for_selected(
        &mut self,
        projects: &[ProjectInfo],
        sessions: &[SessionInfo],
    ) -> bool {
        let Some(target) = self.selected_target(projects, sessions) else {
            self.close_action_menu();
            return false;
        };
        match target {
            SelectedRow::Project(project) => {
                self.context_menu_open = true;
                self.action_cursor = 0;
                self.action_mode = SessionActionMode::RenameProject {
                    project_id: project.id,
                    display_name: project.display_name,
                };
                true
            }
            SelectedRow::Session(session) => {
                if session.archived {
                    return false;
                }
                self.context_menu_open = true;
                self.action_cursor = 0;
                self.action_mode = SessionActionMode::RenameSession {
                    session_id: session.id,
                    title: session.title,
                };
                true
            }
        }
    }

    pub fn close_action_menu(&mut self) {
        self.context_menu_open = false;
        self.action_cursor = 0;
        self.action_mode = SessionActionMode::Menu;
    }

    fn move_archive_down(&mut self, sessions: &[SessionInfo]) {
        let len = archive_stats(sessions).total;
        if len == 0 {
            self.archive_cursor = 0;
            return;
        }
        self.archive_cursor = (self.archive_cursor + 1).min(len - 1);
    }

    fn move_archive_up(&mut self) {
        self.archive_cursor = self.archive_cursor.saturating_sub(1);
    }

    fn selected_archived_session<'a>(
        &self,
        sessions: &'a [SessionInfo],
    ) -> Option<&'a SessionInfo> {
        archived_sessions(sessions)
            .get(self.archive_cursor)
            .copied()
    }

    fn handle_archive_manager_key(&mut self, ctx: &EventContext, code: KeyCode) -> Vec<Command> {
        match code {
            KeyCode::Esc | KeyCode::Char('x') => {
                self.close_archive_manager();
                Vec::new()
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_archive_down(ctx.sessions);
                Vec::new()
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_archive_up();
                Vec::new()
            }
            KeyCode::Enter | KeyCode::Char('r') | KeyCode::Char('R') => {
                let command = self.selected_archived_session(ctx.sessions).map(|session| {
                    Command::RestoreSession {
                        session_id: session.id.clone(),
                    }
                });
                self.close_archive_manager();
                command.into_iter().collect()
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                let command = self.selected_archived_session(ctx.sessions).map(|session| {
                    Command::DeleteSession {
                        session_id: session.id.clone(),
                    }
                });
                self.close_archive_manager();
                command.into_iter().collect()
            }
            _ => Vec::new(),
        }
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

    fn available_actions_for_project(_project: &ProjectInfo) -> &'static [SessionAction] {
        &[
            SessionAction::Rename,
            SessionAction::RemoveProject,
            SessionAction::MoveProjectUp,
            SessionAction::MoveProjectDown,
            SessionAction::ToggleExpanded,
            SessionAction::NewDraft,
            SessionAction::NewWorktree,
            SessionAction::GitStatus,
            SessionAction::InitGit,
            SessionAction::Instructions,
        ]
    }

    fn available_actions(
        &self,
        projects: &[ProjectInfo],
        sessions: &[SessionInfo],
    ) -> &'static [SessionAction] {
        match self.selected_target(projects, sessions) {
            Some(SelectedRow::Project(project)) => Self::available_actions_for_project(&project),
            Some(SelectedRow::Session(session)) => Self::available_actions_for(&session),
            None => &[],
        }
    }

    fn current_action(
        &self,
        projects: &[ProjectInfo],
        sessions: &[SessionInfo],
    ) -> Option<SessionAction> {
        self.available_actions(projects, sessions)
            .get(self.action_cursor)
            .copied()
    }

    fn move_action_down(&mut self, projects: &[ProjectInfo], sessions: &[SessionInfo]) {
        let len = self.available_actions(projects, sessions).len();
        if len == 0 {
            self.action_cursor = 0;
            return;
        }
        self.action_cursor = (self.action_cursor + 1).min(len - 1);
    }

    fn move_action_up(&mut self) {
        self.action_cursor = self.action_cursor.saturating_sub(1);
    }

    fn activate_action(&mut self, action: SessionAction, target: SelectedRow) -> Vec<Command> {
        match (action, target) {
            (SessionAction::Rename, SelectedRow::Project(project)) => {
                self.action_mode = SessionActionMode::RenameProject {
                    project_id: project.id,
                    display_name: project.display_name,
                };
                Vec::new()
            }
            (SessionAction::Rename, SelectedRow::Session(session)) => {
                if !session.archived {
                    self.action_mode = SessionActionMode::RenameSession {
                        session_id: session.id,
                        title: session.title,
                    };
                }
                Vec::new()
            }
            (SessionAction::Archive, SelectedRow::Session(session)) => {
                self.close_action_menu();
                vec![Command::ArchiveSession {
                    session_id: session.id,
                }]
            }
            (SessionAction::Restore, SelectedRow::Session(session)) => {
                self.close_action_menu();
                vec![Command::RestoreSession {
                    session_id: session.id,
                }]
            }
            (SessionAction::Delete, SelectedRow::Session(session)) => {
                self.close_action_menu();
                vec![Command::DeleteSession {
                    session_id: session.id,
                }]
            }
            (SessionAction::RemoveProject, SelectedRow::Project(project)) => {
                self.close_action_menu();
                vec![Command::RemoveProject {
                    project_id: project.id,
                }]
            }
            (SessionAction::MoveProjectUp, SelectedRow::Project(project)) => {
                self.close_action_menu();
                vec![Command::MoveProject {
                    project_id: project.id,
                    direction: -1,
                }]
            }
            (SessionAction::MoveProjectDown, SelectedRow::Project(project)) => {
                self.close_action_menu();
                vec![Command::MoveProject {
                    project_id: project.id,
                    direction: 1,
                }]
            }
            (SessionAction::ToggleExpanded, SelectedRow::Project(project)) => {
                self.close_action_menu();
                vec![Command::SetProjectExpanded {
                    project_id: project.id,
                    expanded: !project.expanded,
                }]
            }
            (SessionAction::NewDraft, SelectedRow::Project(project)) => {
                self.close_action_menu();
                vec![Command::CreateProjectDraftSession {
                    project_id: project.id,
                }]
            }
            (SessionAction::NewDraft, SelectedRow::Session(session)) => {
                self.close_action_menu();
                session
                    .project_id
                    .map(|project_id| Command::CreateProjectDraftSession { project_id })
                    .into_iter()
                    .collect()
            }
            (SessionAction::NewWorktree, SelectedRow::Project(project)) => {
                self.action_mode = SessionActionMode::Worktree {
                    project_id: project.id,
                    branch_name: String::new(),
                };
                Vec::new()
            }
            (SessionAction::NewWorktree, SelectedRow::Session(session)) => {
                if let Some(project_id) = session.project_id {
                    self.action_mode = SessionActionMode::Worktree {
                        project_id,
                        branch_name: String::new(),
                    };
                }
                Vec::new()
            }
            (SessionAction::GitStatus, SelectedRow::Project(project)) => {
                self.close_action_menu();
                vec![Command::RefreshProjectGitStatus {
                    project_id: project.id,
                }]
            }
            (SessionAction::InitGit, SelectedRow::Project(project)) => {
                self.close_action_menu();
                vec![Command::InitProjectGit {
                    project_id: project.id,
                }]
            }
            (SessionAction::Instructions, SelectedRow::Project(project)) => {
                self.close_action_menu();
                vec![Command::ShowProjectInstructions {
                    project_id: project.id,
                }]
            }
            _ => Vec::new(),
        }
    }

    fn handle_menu_key(&mut self, ctx: &EventContext, code: KeyCode) -> Vec<Command> {
        match code {
            KeyCode::Esc => {
                self.close_action_menu();
                Vec::new()
            }
            KeyCode::Char('x') => {
                self.open_action_menu(ctx.projects, ctx.sessions);
                Vec::new()
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_action_down(ctx.projects, ctx.sessions);
                Vec::new()
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_action_up();
                Vec::new()
            }
            KeyCode::Enter => {
                let Some(target) = self.selected_target(ctx.projects, ctx.sessions) else {
                    self.close_action_menu();
                    return Vec::new();
                };
                let Some(action) = self.current_action(ctx.projects, ctx.sessions) else {
                    return Vec::new();
                };
                self.activate_action(action, target)
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                let Some(target) = self.selected_target(ctx.projects, ctx.sessions) else {
                    self.close_action_menu();
                    return Vec::new();
                };
                let action = match &target {
                    SelectedRow::Session(session) if session.archived => SessionAction::Restore,
                    _ => SessionAction::Rename,
                };
                self.activate_action(action, target)
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                let Some(target) = self.selected_target(ctx.projects, ctx.sessions) else {
                    self.close_action_menu();
                    return Vec::new();
                };
                match &target {
                    SelectedRow::Session(session) if !session.archived => {
                        self.activate_action(SessionAction::Archive, target)
                    }
                    _ => Vec::new(),
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                let Some(target) = self.selected_target(ctx.projects, ctx.sessions) else {
                    self.close_action_menu();
                    return Vec::new();
                };
                match &target {
                    SelectedRow::Project(_) => {
                        self.activate_action(SessionAction::RemoveProject, target)
                    }
                    SelectedRow::Session(session) if session.archived => {
                        self.activate_action(SessionAction::Delete, target)
                    }
                    _ => Vec::new(),
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                let Some(target @ SelectedRow::Project(_)) =
                    self.selected_target(ctx.projects, ctx.sessions)
                else {
                    return Vec::new();
                };
                self.activate_action(SessionAction::ToggleExpanded, target)
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let Some(target) = self.selected_target(ctx.projects, ctx.sessions) else {
                    self.close_action_menu();
                    return Vec::new();
                };
                match &target {
                    SelectedRow::Project(_) => {
                        self.activate_action(SessionAction::NewDraft, target)
                    }
                    SelectedRow::Session(session)
                        if session.project_id.is_some() && !session.archived =>
                    {
                        self.activate_action(SessionAction::NewDraft, target)
                    }
                    _ => Vec::new(),
                }
            }
            KeyCode::Char('w') | KeyCode::Char('W') => {
                let Some(target) = self.selected_target(ctx.projects, ctx.sessions) else {
                    self.close_action_menu();
                    return Vec::new();
                };
                match &target {
                    SelectedRow::Project(_) => {
                        self.activate_action(SessionAction::NewWorktree, target)
                    }
                    SelectedRow::Session(session)
                        if session.project_id.is_some() && !session.archived =>
                    {
                        self.activate_action(SessionAction::NewWorktree, target)
                    }
                    _ => Vec::new(),
                }
            }
            KeyCode::Char('g') | KeyCode::Char('G') => {
                let Some(target @ SelectedRow::Project(_)) =
                    self.selected_target(ctx.projects, ctx.sessions)
                else {
                    return Vec::new();
                };
                self.activate_action(SessionAction::GitStatus, target)
            }
            KeyCode::Char('i') => {
                let Some(target @ SelectedRow::Project(_)) =
                    self.selected_target(ctx.projects, ctx.sessions)
                else {
                    return Vec::new();
                };
                self.activate_action(SessionAction::InitGit, target)
            }
            KeyCode::Char('I') => {
                let Some(target @ SelectedRow::Project(_)) =
                    self.selected_target(ctx.projects, ctx.sessions)
                else {
                    return Vec::new();
                };
                self.activate_action(SessionAction::Instructions, target)
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
                    SessionActionMode::RenameSession { session_id, title } => {
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
                    SessionActionMode::RenameProject {
                        project_id,
                        display_name,
                    } => {
                        let display_name = display_name.trim().to_string();
                        if display_name.is_empty() {
                            None
                        } else {
                            Some(Command::RenameProject {
                                project_id: project_id.clone(),
                                display_name,
                            })
                        }
                    }
                    SessionActionMode::Menu | SessionActionMode::Worktree { .. } => None,
                };
                self.close_action_menu();
                command.into_iter().collect()
            }
            KeyCode::Backspace => {
                match &mut self.action_mode {
                    SessionActionMode::RenameSession { title, .. } => {
                        title.pop();
                    }
                    SessionActionMode::RenameProject { display_name, .. } => {
                        display_name.pop();
                    }
                    SessionActionMode::Menu | SessionActionMode::Worktree { .. } => {}
                }
                Vec::new()
            }
            KeyCode::Char(c) => {
                match &mut self.action_mode {
                    SessionActionMode::RenameSession { title, .. } => {
                        title.push(c);
                    }
                    SessionActionMode::RenameProject { display_name, .. } => {
                        display_name.push(c);
                    }
                    SessionActionMode::Menu | SessionActionMode::Worktree { .. } => {}
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
                    SessionActionMode::Menu
                    | SessionActionMode::RenameSession { .. }
                    | SessionActionMode::RenameProject { .. } => None,
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

    pub fn render_action_overlay(
        &self,
        area: Rect,
        frame: &mut Frame,
        projects: &[ProjectInfo],
        sessions: &[SessionInfo],
    ) {
        if self.archive_manager_open {
            render_archive_manager(area, frame, self, projects, sessions);
            return;
        }
        if !self.context_menu_open {
            return;
        }
        render_session_action_overlay(area, frame, self, projects, sessions);
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

pub fn session_list_rows(
    projects: &[ProjectInfo],
    sessions: &[SessionInfo],
) -> Vec<SessionListRow> {
    let mut rows = Vec::new();
    for project in projects {
        rows.push(SessionListRow::Project(project.id.clone()));
        if project.expanded {
            rows.extend(
                sessions
                    .iter()
                    .filter(|session| {
                        !session.archived && session.project_id.as_ref() == Some(&project.id)
                    })
                    .map(|session| SessionListRow::Session(session.id.clone())),
            );
        }
    }

    rows.extend(
        sessions
            .iter()
            .filter(|session| {
                !session.archived
                    && session
                        .project_id
                        .as_ref()
                        .is_none_or(|project_id| !project_exists(projects, project_id))
            })
            .map(|session| SessionListRow::Session(session.id.clone())),
    );
    rows
}

fn project_exists(projects: &[ProjectInfo], project_id: &ProjectId) -> bool {
    projects.iter().any(|project| &project.id == project_id)
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

fn archived_sessions(sessions: &[SessionInfo]) -> Vec<&SessionInfo> {
    sessions.iter().filter(|session| session.archived).collect()
}

fn archive_stats(sessions: &[SessionInfo]) -> ArchiveStats {
    let mut project_ids: Vec<ProjectId> = Vec::new();
    let mut total = 0;
    for session in sessions.iter().filter(|session| session.archived) {
        total += 1;
        if let Some(project_id) = &session.project_id {
            if !project_ids.iter().any(|existing| existing == project_id) {
                project_ids.push(project_id.clone());
            }
        }
    }
    ArchiveStats {
        total,
        projects: project_ids.len(),
    }
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

fn render_archive_manager(
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

fn render_session_action_overlay(
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

impl Component for SessionsPanel {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };

        if self.archive_manager_open {
            let commands = self.handle_archive_manager_key(ctx, key.code);
            return (Vec::new(), commands);
        }

        if !self.context_menu_open {
            if matches!(key.code, KeyCode::Char('x')) {
                self.open_action_menu(ctx.projects, ctx.sessions);
            }
            return (Vec::new(), Vec::new());
        }

        let commands = match self.action_mode {
            SessionActionMode::Menu => self.handle_menu_key(ctx, key.code),
            SessionActionMode::RenameSession { .. } | SessionActionMode::RenameProject { .. } => {
                self.handle_rename_key(key.code)
            }
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
    use agent_core::{ProjectGitStatus, ProjectGitStatusKind, ProjectId, ProjectSessionVisibility};
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
            git_status: None,
            instruction_summary: None,
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
        projects: &'a [ProjectInfo],
        sessions: &'a [SessionInfo],
        current_session_id: &'a Option<SessionId>,
        workspace_id: &'a agent_core::WorkspaceId,
        projection: &'a agent_core::projection::SessionProjection,
    ) -> EventContext<'a> {
        EventContext {
            focus: crate::components::FocusTarget::Sessions,
            current_session: projection,
            projects,
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
    fn filtered_sessions_excludes_archived_sessions() {
        let panel = SessionsPanel::new();
        let sessions = vec![
            make_session("visible", SessionState::Active, false),
            make_archived_session("archived"),
        ];

        let filtered = panel.filtered_sessions(&sessions);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "visible");
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
            &ctx(&[], &sessions, &current, &workspace, &projection),
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
            &ctx(&[], &sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('a')),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::ArchiveSession { session_id }] if session_id == &sessions[1].id
        ));
        assert!(!panel.context_menu_open);
    }

    #[test]
    fn archive_manager_emits_restore_for_archived_session() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![
            make_session("main", SessionState::Active, false),
            make_archived_session("old"),
        ];
        panel.open_archive_manager(&sessions);
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();
        let current = Some(sessions[0].id.clone());

        let (_, commands) = panel.handle_event(
            &ctx(&[], &sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('r')),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::RestoreSession { session_id }] if session_id == &sessions[1].id
        ));
        assert!(!panel.archive_manager_open);
    }

    #[test]
    fn archive_manager_emits_delete_for_archived_session() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![make_archived_session("old")];
        panel.open_archive_manager(&sessions);
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();
        let current = None;

        let (_, commands) = panel.handle_event(
            &ctx(&[], &sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('d')),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::DeleteSession { session_id }] if session_id == &sessions[0].id
        ));
        assert!(!panel.archive_manager_open);
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
            &ctx(&[], &sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('r')),
        );
        assert!(commands.is_empty());
        let (_, commands) = panel.handle_event(
            &ctx(&[], &sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('!')),
        );
        assert!(commands.is_empty());
        let (_, commands) = panel.handle_event(
            &ctx(&[], &sessions, &current, &workspace, &projection),
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
    fn session_list_rows_excludes_archived_sessions_from_primary_list() {
        let active = make_session("active", SessionState::Idle, false);
        let archived = make_archived_session("archived");
        let rows = session_list_rows(&[], &[active.clone(), archived]);

        assert_eq!(rows, vec![SessionListRow::Session(active.id)]);
    }

    #[test]
    fn archive_stats_count_archived_sessions_and_projects() {
        let project = make_project("alpha");
        let mut project_archived =
            make_project_session("archived project", project.id.clone(), Some("main"), None);
        project_archived.archived = true;
        let sessions = vec![
            make_session("active", SessionState::Idle, false),
            project_archived,
            make_archived_session("loose archived"),
        ];

        let stats = archive_stats(&sessions);

        assert_eq!(stats.total, 2);
        assert_eq!(stats.projects, 1);
    }

    #[test]
    fn render_archive_manager_shows_stats_and_archived_rows() {
        let project = make_project("alpha");
        let mut project_archived =
            make_project_session("archived project", project.id.clone(), Some("main"), None);
        project_archived.archived = true;
        let sessions = vec![project_archived, make_archived_session("loose archived")];
        let mut panel = SessionsPanel::new();
        panel.open_archive_manager(&sessions);

        let backend = ratatui::backend::TestBackend::new(80, 20);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                panel.render_action_overlay(
                    frame.area(),
                    frame,
                    std::slice::from_ref(&project),
                    &sessions,
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

        assert!(text.contains("Archive Manager"));
        assert!(text.contains("Total: 2"));
        assert!(text.contains("Projects: 1"));
        assert!(text.contains("archived project"));
        assert!(text.contains("alpha"));
        assert!(text.contains("[Enter/r] restore"));
    }

    #[test]
    fn archive_manager_restore_shortcut_emits_restore_for_selected_archived_session() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![
            make_archived_session("first"),
            make_archived_session("second"),
        ];
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();

        assert!(panel.open_archive_manager(&sessions));
        panel.archive_cursor = 1;
        let (_, commands) = panel.handle_event(
            &ctx(&[], &sessions, &None, &workspace, &projection),
            &key(KeyCode::Char('r')),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::RestoreSession { session_id }] if session_id == &sessions[1].id
        ));
        assert!(!panel.archive_manager_open);
    }

    #[test]
    fn archive_manager_delete_shortcut_emits_delete_for_selected_archived_session() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![make_archived_session("archived")];
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();

        assert!(panel.open_archive_manager(&sessions));
        let (_, commands) = panel.handle_event(
            &ctx(&[], &sessions, &None, &workspace, &projection),
            &key(KeyCode::Char('d')),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::DeleteSession { session_id }] if session_id == &sessions[0].id
        ));
        assert!(!panel.archive_manager_open);
    }

    #[test]
    fn project_row_context_menu_emits_create_draft_for_empty_project() {
        let project = make_project("empty");
        let projects = vec![project.clone()];
        let sessions = Vec::new();
        let mut panel = SessionsPanel::new();
        panel.context_menu_open = true;
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();
        let current = None;

        let (_, commands) = panel.handle_event(
            &ctx(&projects, &sessions, &current, &workspace, &projection),
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
    fn project_row_context_menu_emits_expand_persistence_command() {
        let mut project = make_project("collapsed");
        project.expanded = false;
        let projects = vec![project.clone()];
        let sessions = Vec::new();
        let mut panel = SessionsPanel::new();
        panel.context_menu_open = true;
        let workspace = agent_core::WorkspaceId::new();
        let projection = agent_core::projection::SessionProjection::default();
        let current = None;

        let (_, commands) = panel.handle_event(
            &ctx(&projects, &sessions, &current, &workspace, &projection),
            &key(KeyCode::Char('e')),
        );

        assert!(matches!(
            commands.as_slice(),
            [Command::SetProjectExpanded { project_id, expanded }]
                if project_id == &project.id && *expanded
        ));
        assert!(!panel.context_menu_open);
    }

    #[test]
    fn render_project_rows_show_git_branch_dirty_and_missing_path() {
        let mut clean = make_project("clean");
        clean.git_status = Some(ProjectGitStatus {
            kind: ProjectGitStatusKind::Clean,
            branch: Some("main".into()),
            worktree_path: clean.root_path.clone(),
            message: None,
        });
        let mut dirty = make_project("changed");
        dirty.git_status = Some(ProjectGitStatus {
            kind: ProjectGitStatusKind::Dirty,
            branch: Some("feat/tui".into()),
            worktree_path: dirty.root_path.clone(),
            message: None,
        });
        let mut missing = make_project("missing");
        missing.git_status = Some(ProjectGitStatus {
            kind: ProjectGitStatusKind::MissingPath,
            branch: None,
            worktree_path: missing.root_path.clone(),
            message: Some("path does not exist".into()),
        });
        let mut panel_state = ListState::default();

        let backend = ratatui::backend::TestBackend::new(96, 8);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                render_sessions(
                    frame.area(),
                    frame,
                    &[clean, dirty, missing],
                    &[],
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

        assert!(text.contains("main"));
        assert!(text.contains("dirty"));
        assert!(text.contains("feat/tui"));
        assert!(text.contains("missing path"));
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
            &ctx(
                std::slice::from_ref(&project),
                &sessions,
                &current,
                &workspace,
                &projection,
            ),
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
            &ctx(
                std::slice::from_ref(&project),
                &sessions,
                &current,
                &workspace,
                &projection,
            ),
            &key(KeyCode::Char('w')),
        );
        assert!(commands.is_empty());

        for ch in "feat/tui-parity".chars() {
            let (_, commands) = panel.handle_event(
                &ctx(
                    std::slice::from_ref(&project),
                    &sessions,
                    &current,
                    &workspace,
                    &projection,
                ),
                &key(KeyCode::Char(ch)),
            );
            assert!(commands.is_empty());
        }

        let (_, commands) = panel.handle_event(
            &ctx(
                std::slice::from_ref(&project),
                &sessions,
                &current,
                &workspace,
                &projection,
            ),
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

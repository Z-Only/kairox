//! Keyboard input handling for the sessions panel.
//!
//! Extracted from [`super::state`] to keep the data model separate from
//! input-driven mutations.

use crossterm::event::KeyCode;

use crate::components::{Command, EventContext};

use super::state::{
    archive_stats, archived_sessions, SelectedRow, SessionAction, SessionActionMode, SessionsPanel,
};

impl SessionsPanel {
    fn move_archive_down(&mut self, sessions: &[crate::components::SessionInfo]) {
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
        sessions: &'a [crate::components::SessionInfo],
    ) -> Option<&'a crate::components::SessionInfo> {
        archived_sessions(sessions)
            .get(self.archive_cursor)
            .copied()
    }

    pub(super) fn handle_archive_manager_key(
        &mut self,
        ctx: &EventContext,
        code: KeyCode,
    ) -> Vec<Command> {
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
                command.into_iter().collect()
            }
            _ => Vec::new(),
        }
    }

    fn current_action(
        &self,
        projects: &[crate::components::ProjectInfo],
        sessions: &[crate::components::SessionInfo],
    ) -> Option<SessionAction> {
        self.available_actions(projects, sessions)
            .get(self.action_cursor)
            .copied()
    }

    fn move_action_down(
        &mut self,
        projects: &[crate::components::ProjectInfo],
        sessions: &[crate::components::SessionInfo],
    ) {
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
                vec![Command::DeleteSession {
                    session_id: session.id,
                }]
            }
            (SessionAction::RemoveProject, SelectedRow::Project(project)) => {
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

    pub(super) fn handle_menu_key(&mut self, ctx: &EventContext, code: KeyCode) -> Vec<Command> {
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

    pub(super) fn handle_rename_key(&mut self, code: KeyCode) -> Vec<Command> {
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

    pub(super) fn handle_worktree_key(&mut self, code: KeyCode) -> Vec<Command> {
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
}

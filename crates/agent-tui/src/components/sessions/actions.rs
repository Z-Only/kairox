//! Session action handlers — methods that mutate [`SessionsPanel`] state in
//! response to user-initiated actions (rename, delete, archive, worktree, etc.)
//!
//! Key-event dispatch lives in [`super::keys`]. This file contains the pure
//! business-logic mutations: menu open/close, cursor movement, target resolution,
//! and action activation.

use super::state::{
    archive_stats, archived_sessions, SelectedRow, SessionAction, SessionActionMode, SessionsPanel,
};
use crate::components::{Command, ProjectInfo, SessionInfo};

impl SessionsPanel {
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

    pub(super) fn move_archive_down(&mut self, sessions: &[SessionInfo]) {
        let len = archive_stats(sessions).total;
        if len == 0 {
            self.archive_cursor = 0;
            return;
        }
        self.archive_cursor = (self.archive_cursor + 1).min(len - 1);
    }

    pub(super) fn move_archive_up(&mut self) {
        self.archive_cursor = self.archive_cursor.saturating_sub(1);
    }

    pub(super) fn selected_archived_session<'a>(
        &self,
        sessions: &'a [SessionInfo],
    ) -> Option<&'a SessionInfo> {
        archived_sessions(sessions)
            .get(self.archive_cursor)
            .copied()
    }

    pub(super) fn move_action_down(&mut self, projects: &[ProjectInfo], sessions: &[SessionInfo]) {
        let len = self.available_actions(projects, sessions).len();
        if len == 0 {
            self.action_cursor = 0;
            return;
        }
        self.action_cursor = (self.action_cursor + 1).min(len - 1);
    }

    pub(super) fn move_action_up(&mut self) {
        self.action_cursor = self.action_cursor.saturating_sub(1);
    }

    pub(super) fn activate_action(
        &mut self,
        action: SessionAction,
        target: SelectedRow,
    ) -> Vec<Command> {
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
}

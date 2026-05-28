//! Sessions panel state: data model, selection helpers, and action lookups.
//!
//! Key handling lives in [`super::keys`]. Render code lives in
//! [`super::render`]. The split mirrors `mcp_overlay` / `skills_overlay`.

use crate::components::{ProjectInfo, SessionInfo};
use agent_core::{ProjectId, SessionId};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
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
pub(super) enum SessionActionMode {
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
    pub(super) focused: bool,
    pub state: ListState,
    pub context_menu_open: bool,
    pub archive_manager_open: bool,
    pub search_query: Option<String>,
    pub(super) action_cursor: usize,
    pub(super) archive_cursor: usize,
    pub(super) action_mode: SessionActionMode,
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
pub(super) enum SelectedRow {
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

    pub(super) fn selected_target(
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

    pub(super) fn available_actions(
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

    pub fn render_action_overlay(
        &self,
        area: Rect,
        frame: &mut Frame,
        projects: &[ProjectInfo],
        sessions: &[SessionInfo],
    ) {
        if self.archive_manager_open {
            super::render::render_archive_manager(area, frame, self, projects, sessions);
            return;
        }
        if !self.context_menu_open {
            return;
        }
        super::render::render_session_action_overlay(area, frame, self, projects, sessions);
    }
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

pub(super) fn project_exists(projects: &[ProjectInfo], project_id: &ProjectId) -> bool {
    projects.iter().any(|project| &project.id == project_id)
}

pub(super) fn archived_sessions(sessions: &[SessionInfo]) -> Vec<&SessionInfo> {
    sessions.iter().filter(|session| session.archived).collect()
}

pub(super) fn archive_stats(sessions: &[SessionInfo]) -> ArchiveStats {
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

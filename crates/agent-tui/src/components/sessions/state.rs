//! Sessions panel state: data model, key handling, and selection helpers.
//!
//! Render code lives in [`super::render`]. Action handler methods live in
//! [`super::actions`]. The split mirrors `mcp_overlay` / `skills_overlay`.

use crate::components::{ProjectInfo, SessionInfo};
use agent_core::SessionId;
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use ratatui::Frame;

// Re-export all types from the new `types` submodule so that existing
// `super::state::Foo` paths in sibling modules (keys, actions, render) and
// `pub use state::Foo` in mod.rs continue to resolve without changes.
pub use super::types::{
    session_list_rows, ArchiveStats, SessionAction, SessionListRow,
};
pub(super) use super::types::{
    archive_stats, archived_sessions, project_exists, SelectedRow, SessionActionMode,
};

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

    pub fn is_overlay_open(&self) -> bool {
        self.context_menu_open || self.archive_manager_open
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

    pub(super) fn current_action(
        &self,
        projects: &[ProjectInfo],
        sessions: &[SessionInfo],
    ) -> Option<SessionAction> {
        self.available_actions(projects, sessions)
            .get(self.action_cursor)
            .copied()
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

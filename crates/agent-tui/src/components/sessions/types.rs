//! Data types for the sessions panel -- action/mode/row enums and stats struct
//! used across the panel submodules.

use crate::components::{ProjectInfo, SessionInfo};
use agent_core::{ProjectId, SessionId};

/// Action variants for session management.
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

// ---------------------------------------------------------------------------
// Data-adjacent free functions (no SessionsPanel dependency)
// ---------------------------------------------------------------------------

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

use std::sync::Arc;

use agent_core::{AppFacade, ProjectGitStatus, ProjectMeta, SessionMeta};
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

use crate::app::App;
use crate::components::{self, ProjectInfo, SessionInfo, SessionState};

pub fn project_info_from_meta(project: ProjectMeta) -> ProjectInfo {
    ProjectInfo {
        id: project.project_id,
        display_name: project.display_name,
        root_path: project.root_path,
        expanded: project.expanded,
        git_status: None,
        instruction_summary: None,
    }
}

pub fn session_info_from_meta(
    session: SessionMeta,
    archived: bool,
    current_session_id: &Option<agent_core::SessionId>,
) -> SessionInfo {
    let state = if current_session_id.as_ref() == Some(&session.session_id) {
        SessionState::Active
    } else {
        SessionState::Idle
    };
    SessionInfo {
        id: session.session_id,
        title: session.title,
        model_profile: session.model_profile,
        state,
        pinned: false,
        archived,
        project_id: session.project_id,
        worktree_path: session.worktree_path,
        branch: session.branch,
        visibility: session.visibility,
    }
}

pub(super) async fn refresh_project_sessions_for_project(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    project_id: &agent_core::ProjectId,
) -> agent_core::Result<()> {
    let sessions = runtime.list_project_sessions(project_id.clone()).await?;
    let current_session_id = app.current_session_id.clone();
    let next_project_sessions: Vec<SessionInfo> = sessions
        .into_iter()
        .map(|session| session_info_from_meta(session, false, &current_session_id))
        .collect();

    app.state
        .sessions
        .retain(|session| session.archived || session.project_id.as_ref() != Some(project_id));
    app.state.sessions.extend(next_project_sessions);
    normalize_session_states(app);
    app.state.render_scheduler.mark_dirty();
    Ok(())
}

pub(super) async fn refresh_session_git_metadata(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    session_id: &agent_core::SessionId,
) {
    if let Ok(status) = runtime.get_session_git_status(session_id.clone()).await {
        apply_session_git_status(app, session_id, &status);
    }
}

pub(super) fn apply_session_git_status(
    app: &mut App,
    session_id: &agent_core::SessionId,
    status: &ProjectGitStatus,
) {
    if let Some(session) = app
        .state
        .sessions
        .iter_mut()
        .find(|session| &session.id == session_id)
    {
        session.branch = status.branch.clone();
        session.worktree_path = Some(status.worktree_path.clone());
    }
    app.sync_status_bar();
    app.state.render_scheduler.mark_dirty();
}

fn normalize_session_states(app: &mut App) {
    let current_session_id = app.current_session_id.clone();
    for session in &mut app.state.sessions {
        if current_session_id.as_ref() == Some(&session.id) && !session.archived {
            session.state = SessionState::Active;
        } else if matches!(session.state, SessionState::Active) {
            session.state = SessionState::Idle;
        }
    }
}

pub(super) fn select_session_row(app: &mut App, session_id: &agent_core::SessionId) {
    if let Some(index) = components::sessions::session_list_rows(
        &app.state.projects,
        &app.state.sessions,
    )
    .iter()
    .position(
        |row| matches!(row, components::sessions::SessionListRow::Session(id) if id == session_id),
    ) {
        app.sessions.state.select(Some(index));
    }
}

pub(super) fn clamp_session_selection(app: &mut App) {
    let len =
        components::sessions::session_list_rows(&app.state.projects, &app.state.sessions).len();
    if len == 0 {
        app.sessions.state.select(None);
        return;
    }
    let selected = app.sessions.state.selected().unwrap_or(0).min(len - 1);
    app.sessions.state.select(Some(selected));
}

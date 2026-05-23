use std::sync::Arc;

use agent_core::{AppFacade, ProjectSessionVisibility, StartSessionRequest};
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

use crate::app::App;
use crate::components::{SessionInfo, SessionState};

use super::super::push_status_error;
use super::state::{
    clamp_session_selection, refresh_project_sessions_for_project, refresh_session_git_metadata,
    select_session_row,
};

pub(super) async fn start_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    workspace_id: agent_core::WorkspaceId,
    model_profile: String,
) {
    match runtime
        .start_session(StartSessionRequest {
            workspace_id,
            model_profile: model_profile.clone(),
            permission_mode: None,
        })
        .await
    {
        Ok(session_id) => {
            app.current_session_id = Some(session_id.clone());
            app.state.sessions.push(SessionInfo {
                id: session_id.clone(),
                title: format!("Session using {model_profile}"),
                model_profile,
                state: SessionState::Idle,
                pinned: false,
                archived: false,
                project_id: None,
                worktree_path: None,
                branch: None,
                visibility: None,
            });
            app.state.current_session = agent_core::projection::SessionProjection::default();
            app.domain_events.clear();
            restore_session_draft(runtime.store(), app, &session_id).await;
            app.state.render_scheduler.reset();
            app.sessions
                .state
                .select(Some(app.state.sessions.len() - 1));
        }
        Err(e) => {
            push_status_error(app, format!("[start session error: {e}]"));
        }
    }
}

pub(super) async fn switch_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    session_id: agent_core::SessionId,
) {
    switch_app_to_session(runtime, app, session_id).await;
}

pub(super) async fn rename_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    session_id: agent_core::SessionId,
    title: String,
) {
    match runtime.rename_session(&session_id, title.clone()).await {
        Ok(()) => {
            if let Some(session) = app.state.sessions.iter_mut().find(|s| s.id == session_id) {
                session.title = title;
            }
            app.state.render_scheduler.mark_dirty();
        }
        Err(e) => push_status_error(app, format!("[rename session error: {e}]")),
    }
}

pub(super) async fn archive_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    session_id: agent_core::SessionId,
) {
    match runtime.soft_delete_session(&session_id).await {
        Ok(()) => {
            if let Some(session) = app.state.sessions.iter_mut().find(|s| s.id == session_id) {
                session.archived = true;
                session.state = SessionState::Idle;
                session.visibility = Some(ProjectSessionVisibility::Archived);
            }
            if app.current_session_id.as_ref() == Some(&session_id) {
                switch_to_first_active_session(runtime, app).await;
            } else {
                clamp_session_selection(app);
                app.state.render_scheduler.mark_dirty();
            }
        }
        Err(e) => push_status_error(app, format!("[archive session error: {e}]")),
    }
}

pub(super) async fn restore_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    session_id: agent_core::SessionId,
) {
    let project_session = app
        .state
        .sessions
        .iter()
        .find(|session| session.id == session_id)
        .and_then(|session| session.project_id.clone());
    let restore_result = if project_session.is_some() {
        match runtime.restore_archived_session(&session_id).await {
            Ok(()) => runtime
                .restore_project_session(session_id.clone())
                .await
                .map(|_| ()),
            Err(error) => Err(error),
        }
    } else {
        runtime.restore_archived_session(&session_id).await
    };
    match restore_result {
        Ok(()) => {
            if let Some(session) = app.state.sessions.iter_mut().find(|s| s.id == session_id) {
                session.archived = false;
                session.state = SessionState::Idle;
                session.visibility = Some(ProjectSessionVisibility::Visible);
            }
            if let Some(project_id) = project_session {
                let _ = refresh_project_sessions_for_project(runtime, app, &project_id).await;
                refresh_session_git_metadata(runtime, app, &session_id).await;
            }
            select_session_row(app, &session_id);
            app.state.render_scheduler.mark_dirty();
        }
        Err(e) => push_status_error(app, format!("[restore session error: {e}]")),
    }
}

pub(super) async fn delete_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    session_id: agent_core::SessionId,
) {
    match runtime.permanently_delete_session(&session_id).await {
        Ok(()) => {
            app.state.sessions.retain(|s| s.id != session_id);
            if app.current_session_id.as_ref() == Some(&session_id) {
                switch_to_first_active_session(runtime, app).await;
            } else {
                clamp_session_selection(app);
                app.state.render_scheduler.mark_dirty();
            }
        }
        Err(e) => push_status_error(app, format!("[delete session error: {e}]")),
    }
}

pub(super) async fn create_project_draft_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    project_id: agent_core::ProjectId,
) {
    match runtime
        .create_project_draft_session(project_id.clone())
        .await
    {
        Ok(session_id) => {
            let _ = runtime
                .rename_session(&session_id, "New Session".to_string())
                .await;
            match refresh_project_sessions_for_project(runtime, app, &project_id).await {
                Ok(_) => {
                    switch_app_to_session(runtime, app, session_id).await;
                }
                Err(e) => {
                    push_status_error(app, format!("[project session refresh error: {e}]"));
                }
            }
        }
        Err(e) => push_status_error(app, format!("[create project draft error: {e}]")),
    }
}

pub(super) async fn create_project_worktree_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    project_id: agent_core::ProjectId,
    branch_name: String,
) {
    match runtime
        .create_project_worktree_session(project_id.clone(), branch_name.clone())
        .await
    {
        Ok(session_id) => {
            let _ = runtime
                .rename_session(&session_id, format!("New Session ({branch_name})"))
                .await;
            match refresh_project_sessions_for_project(runtime, app, &project_id).await {
                Ok(_) => {
                    refresh_session_git_metadata(runtime, app, &session_id).await;
                    switch_app_to_session(runtime, app, session_id).await;
                }
                Err(e) => {
                    push_status_error(app, format!("[project session refresh error: {e}]"));
                }
            }
        }
        Err(e) => push_status_error(app, format!("[create worktree session error: {e}]")),
    }
}

async fn switch_app_to_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    sid: agent_core::SessionId,
) {
    app.current_session_id = Some(sid.clone());

    for session in &mut app.state.sessions {
        if session.id == sid {
            session.state = SessionState::Active;
        } else if session.state == SessionState::Active {
            session.state = SessionState::Idle;
        }
    }
    select_session_row(app, &sid);

    let projection = runtime.get_session_projection(sid.clone()).await;
    let trace = runtime.get_trace(sid.clone()).await;

    if let Ok(proj) = projection {
        app.state.current_session = proj;
    }
    if let Ok(trc) = trace {
        app.domain_events = trc.into_iter().map(|t| t.event).collect();
    }
    restore_session_draft(runtime.store(), app, &sid).await;

    app.state.render_scheduler.mark_dirty_immediate();
}

async fn switch_to_first_active_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
) {
    if let Some(session_id) = app
        .state
        .sessions
        .iter()
        .find(|session| !session.archived)
        .map(|session| session.id.clone())
    {
        switch_app_to_session(runtime, app, session_id).await;
    } else {
        app.current_session_id = None;
        app.state.current_session = agent_core::projection::SessionProjection::default();
        app.chat.set_draft_text("");
        app.domain_events.clear();
        clamp_session_selection(app);
        app.state.render_scheduler.mark_dirty_immediate();
    }
}

pub async fn restore_session_draft(
    store: &SqliteEventStore,
    app: &mut App,
    sid: &agent_core::SessionId,
) {
    match store.get_draft(sid.as_str()).await {
        Ok(draft) => app.chat.set_draft_text(draft),
        Err(error) => push_status_error(app, format!("[draft load error: {error}]")),
    }
}

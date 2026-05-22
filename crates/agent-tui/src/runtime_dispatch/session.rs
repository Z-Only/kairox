use std::sync::Arc;

use agent_core::{
    AppFacade, ProjectGitStatus, ProjectMeta, ProjectSessionVisibility, SendMessageRequest,
    SessionMeta, StartSessionRequest,
};
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

use crate::app;
use crate::app::App;
use crate::components::{self, Command, ProjectInfo, SessionInfo, SessionState};

use super::{push_status_error, push_status_message};

pub(crate) async fn dispatch(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    command: Command,
) {
    match command {
        Command::SaveDraft {
            session_id,
            draft_text,
        } => {
            if let Err(e) = runtime
                .store()
                .save_draft(session_id.as_str(), &draft_text)
                .await
            {
                push_status_error(app, format!("[draft save error: {e}]"));
            }
        }

        Command::SendMessage {
            workspace_id,
            session_id,
            content,
            attachments,
        } => {
            match runtime
                .send_message(SendMessageRequest {
                    workspace_id,
                    session_id: session_id.clone(),
                    content,
                    attachments,
                })
                .await
            {
                Ok(()) => {
                    let project_id = app
                        .state
                        .sessions
                        .iter()
                        .find(|session| session.id == session_id)
                        .and_then(|session| match session.visibility {
                            Some(ProjectSessionVisibility::DraftHidden) => {
                                session.project_id.clone()
                            }
                            _ => None,
                        });
                    if let Some(project_id) = project_id {
                        let _ =
                            refresh_project_sessions_for_project(runtime, app, &project_id).await;
                    }
                }
                Err(e) => {
                    push_status_error(app, format!("[error: {e}]"));
                }
            }
        }

        Command::SendQueuedMessageNow {
            workspace_id,
            session_id,
            queue_index,
        } => {
            send_queued_message_now(runtime, app, workspace_id, session_id, queue_index).await;
        }

        Command::ApplyQueueAction(action) => {
            let commands = app.apply_queue_action(action);
            for command in commands {
                if let Command::SendQueuedMessageNow {
                    workspace_id,
                    session_id,
                    queue_index,
                } = command
                {
                    send_queued_message_now(runtime, app, workspace_id, session_id, queue_index)
                        .await;
                }
            }
        }

        Command::DecidePermission {
            request_id,
            approved,
        } => {
            if let Err(e) = runtime
                .resolve_permission(
                    &request_id,
                    agent_core::PermissionDecision {
                        request_id: request_id.clone(),
                        approve: approved,
                        reason: None,
                    },
                )
                .await
            {
                push_status_error(app, format!("[permission error: {e}]"));
            }
        }

        Command::CancelSession {
            workspace_id,
            session_id,
        } => {
            if let Err(e) = runtime.cancel_session(workspace_id, session_id).await {
                push_status_error(app, format!("[cancel error: {e}]"));
            }
        }

        Command::RetryTask {
            workspace_id,
            session_id,
            task_id,
        } => {
            if let Err(e) = runtime.retry_task(workspace_id, session_id, task_id).await {
                push_status_error(app, format!("[task retry error: {e}]"));
            }
        }

        Command::CancelTask {
            workspace_id,
            session_id,
            task_id,
        } => {
            if let Err(e) = runtime.cancel_task(workspace_id, session_id, task_id).await {
                push_status_error(app, format!("[task cancel error: {e}]"));
            }
        }

        Command::ClearSessionProjection => {
            app::clear_session_projection(app);
            push_status_message(app, "cleared local conversation projection".to_string());
            app.state.render_scheduler.mark_dirty_immediate();
        }

        Command::StartSession {
            workspace_id: ws_id,
            model_profile: mp,
        } => {
            match runtime
                .start_session(StartSessionRequest {
                    workspace_id: ws_id,
                    model_profile: mp.clone(),
                    permission_mode: None,
                })
                .await
            {
                Ok(session_id) => {
                    app.current_session_id = Some(session_id.clone());
                    app.state.sessions.push(SessionInfo {
                        id: session_id.clone(),
                        title: format!("Session using {mp}"),
                        model_profile: mp,
                        state: SessionState::Idle,
                        pinned: false,
                        archived: false,
                        project_id: None,
                        worktree_path: None,
                        branch: None,
                        visibility: None,
                    });
                    app.state.current_session =
                        agent_core::projection::SessionProjection::default();
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

        Command::SwitchSession { session_id } => {
            switch_app_to_session(runtime, app, session_id).await;
        }

        Command::RenameSession { session_id, title } => {
            match runtime.rename_session(&session_id, title.clone()).await {
                Ok(()) => {
                    if let Some(session) =
                        app.state.sessions.iter_mut().find(|s| s.id == session_id)
                    {
                        session.title = title;
                    }
                    app.state.render_scheduler.mark_dirty();
                }
                Err(e) => push_status_error(app, format!("[rename session error: {e}]")),
            }
        }

        Command::ArchiveSession { session_id } => {
            match runtime.soft_delete_session(&session_id).await {
                Ok(()) => {
                    if let Some(session) =
                        app.state.sessions.iter_mut().find(|s| s.id == session_id)
                    {
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

        Command::RestoreSession { session_id } => {
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
                    if let Some(session) =
                        app.state.sessions.iter_mut().find(|s| s.id == session_id)
                    {
                        session.archived = false;
                        session.state = SessionState::Idle;
                        session.visibility = Some(ProjectSessionVisibility::Visible);
                    }
                    if let Some(project_id) = project_session {
                        let _ =
                            refresh_project_sessions_for_project(runtime, app, &project_id).await;
                        refresh_session_git_metadata(runtime, app, &session_id).await;
                    }
                    select_session_row(app, &session_id);
                    app.state.render_scheduler.mark_dirty();
                }
                Err(e) => push_status_error(app, format!("[restore session error: {e}]")),
            }
        }

        Command::DeleteSession { session_id } => {
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

        Command::CreateProjectDraftSession { project_id } => {
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

        Command::CreateProjectWorktreeSession {
            project_id,
            branch_name,
        } => {
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

        _ => {}
    }
}

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

async fn refresh_project_sessions_for_project(
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

async fn refresh_session_git_metadata(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    session_id: &agent_core::SessionId,
) {
    if let Ok(status) = runtime.get_session_git_status(session_id.clone()).await {
        apply_session_git_status(app, session_id, &status);
    }
}

fn apply_session_git_status(
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

async fn send_queued_message_now(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    workspace_id: agent_core::WorkspaceId,
    session_id: agent_core::SessionId,
    queue_index: usize,
) {
    let Some(queued) = app.chat.queued_message(queue_index).cloned() else {
        app.state.render_scheduler.mark_dirty();
        return;
    };
    match runtime
        .send_message(SendMessageRequest {
            workspace_id,
            session_id: session_id.clone(),
            content: queued.content,
            attachments: queued.attachments,
        })
        .await
    {
        Ok(()) => {
            let project_id = app
                .state
                .sessions
                .iter()
                .find(|session| session.id == session_id)
                .and_then(|session| match session.visibility {
                    Some(ProjectSessionVisibility::DraftHidden) => session.project_id.clone(),
                    _ => None,
                });
            if let Some(project_id) = project_id {
                let _ = refresh_project_sessions_for_project(runtime, app, &project_id).await;
            }
            app.chat.remove_queued_message(queue_index);
            app.state.render_scheduler.mark_dirty();
        }
        Err(e) => {
            push_status_error(app, format!("[queued send error: {e}]"));
        }
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

fn select_session_row(app: &mut App, session_id: &agent_core::SessionId) {
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

fn clamp_session_selection(app: &mut App) {
    let len =
        components::sessions::session_list_rows(&app.state.projects, &app.state.sessions).len();
    if len == 0 {
        app.sessions.state.select(None);
        return;
    }
    let selected = app.sessions.state.selected().unwrap_or(0).min(len - 1);
    app.sessions.state.select(Some(selected));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn restore_session_draft_loads_saved_composer_text() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        store
            .upsert_workspace("wrk_test", "/tmp/kairox")
            .await
            .unwrap();
        let session_id = agent_core::SessionId::from_string("ses_restore".to_string());
        let now = "2026-05-21T00:00:00Z".to_string();
        store
            .upsert_session(&agent_store::SessionRow {
                session_id: session_id.as_str().to_string(),
                workspace_id: "wrk_test".to_string(),
                title: "Restore me".to_string(),
                model_profile: "test".to_string(),
                model_id: None,
                provider: None,
                permission_mode: "suggest".to_string(),
                deleted_at: None,
                created_at: now.clone(),
                updated_at: now,
            })
            .await
            .unwrap();
        store
            .save_draft(session_id.as_str(), "saved draft")
            .await
            .unwrap();
        let mut app = App::new(
            "test",
            agent_tools::PermissionMode::Suggest,
            agent_core::WorkspaceId::from_string("wrk_test".to_string()),
        );
        app.chat.input_content = "old text".to_string();
        app.chat.input_cursor = app.chat.input_content.len();

        restore_session_draft(&store, &mut app, &session_id).await;

        assert_eq!(app.chat.input_content, "saved draft");
        assert_eq!(app.chat.input_cursor, "saved draft".len());
    }

    #[test]
    fn session_git_meta_applies_refreshed_status_to_session() {
        let session_id = agent_core::SessionId::from_string("ses_git".to_string());
        let mut app = App::new(
            "test",
            agent_tools::PermissionMode::Suggest,
            agent_core::WorkspaceId::from_string("wrk_test".to_string()),
        );
        app.current_session_id = Some(session_id.clone());
        app.state.sessions.push(SessionInfo {
            id: session_id.clone(),
            title: "Worktree".to_string(),
            model_profile: "test".to_string(),
            state: SessionState::Active,
            pinned: false,
            archived: false,
            project_id: None,
            worktree_path: None,
            branch: None,
            visibility: Some(ProjectSessionVisibility::Visible),
        });

        apply_session_git_status(
            &mut app,
            &session_id,
            &ProjectGitStatus {
                kind: agent_core::ProjectGitStatusKind::Clean,
                branch: Some("feat/tui".to_string()),
                worktree_path: "/tmp/project/.kairox/worktrees/feat-tui".to_string(),
                message: None,
            },
        );

        let session = app
            .state
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .expect("session");
        assert_eq!(session.branch.as_deref(), Some("feat/tui"));
        assert_eq!(
            session.worktree_path.as_deref(),
            Some("/tmp/project/.kairox/worktrees/feat-tui")
        );
        let metadata = app.current_session_git_metadata();
        assert!(metadata.iter().any(|part| part == "worktrees/feat-tui"));
    }
}

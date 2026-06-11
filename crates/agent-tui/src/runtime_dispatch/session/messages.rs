use std::sync::Arc;

use agent_core::{AppFacade, AttachmentInfo, ProjectSessionVisibility, SendMessageRequest};
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

use crate::app;
use crate::app::App;
use crate::components::{Command, QueueAction};

use super::super::{push_status_error, push_status_message};
use super::state::refresh_project_sessions_for_project;

pub(super) async fn save_draft(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    session_id: agent_core::SessionId,
    draft_text: String,
) {
    if let Err(e) = runtime
        .store()
        .save_draft(session_id.as_str(), &draft_text)
        .await
    {
        push_status_error(app, format!("[draft save error: {e}]"));
    }
}

pub(super) async fn send_message(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    workspace_id: agent_core::WorkspaceId,
    session_id: agent_core::SessionId,
    content: String,
    attachments: Vec<AttachmentInfo>,
) {
    let (content, display_content) = prepare_goal_message_for_dispatch(content);
    match runtime
        .send_message(SendMessageRequest {
            workspace_id,
            session_id: session_id.clone(),
            content,
            display_content,
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
                    Some(ProjectSessionVisibility::DraftHidden) => session.project_id.clone(),
                    _ => None,
                });
            if let Some(project_id) = project_id {
                let _ = refresh_project_sessions_for_project(runtime, app, &project_id).await;
            }
        }
        Err(e) => {
            push_status_error(app, format!("[error: {e}]"));
        }
    }
}

fn prepare_goal_message_for_dispatch(content: String) -> (String, Option<String>) {
    let Some(goal) = content
        .strip_prefix(":goal ")
        .map(str::trim)
        .filter(|goal| !goal.is_empty())
    else {
        return (content, None);
    };

    let model_content = format!(
        "# Goal\n\n{goal}\n\nWork toward this goal until it is complete. Track progress, verify concrete changes, and report blockers explicitly."
    );
    (model_content, Some(content))
}

pub(super) async fn send_queued_message_now(
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
            display_content: None,
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

pub(super) async fn apply_queue_action(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    action: QueueAction,
) {
    let commands = app.apply_queue_action(action);
    for command in commands {
        if let Command::SendQueuedMessageNow {
            workspace_id,
            session_id,
            queue_index,
        } = command
        {
            send_queued_message_now(runtime, app, workspace_id, session_id, queue_index).await;
        }
    }
}

pub(super) async fn decide_permission(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    request_id: String,
    approved: bool,
) {
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

pub(super) async fn cancel_session(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    workspace_id: agent_core::WorkspaceId,
    session_id: agent_core::SessionId,
) {
    if let Err(e) = runtime.cancel_session(workspace_id, session_id).await {
        push_status_error(app, format!("[cancel error: {e}]"));
    }
}

pub(super) async fn retry_task(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    workspace_id: agent_core::WorkspaceId,
    session_id: agent_core::SessionId,
    task_id: agent_core::TaskId,
) {
    if let Err(e) = runtime.retry_task(workspace_id, session_id, task_id).await {
        push_status_error(app, format!("[task retry error: {e}]"));
    }
}

pub(super) async fn cancel_task(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    workspace_id: agent_core::WorkspaceId,
    session_id: agent_core::SessionId,
    task_id: agent_core::TaskId,
) {
    if let Err(e) = runtime.cancel_task(workspace_id, session_id, task_id).await {
        push_status_error(app, format!("[task cancel error: {e}]"));
    }
}

pub(super) fn clear_session_projection(app: &mut App) {
    app::clear_session_projection(app);
    push_status_message(app, "cleared local conversation projection".to_string());
    app.state.render_scheduler.mark_dirty_immediate();
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;

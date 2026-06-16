mod lifecycle;
mod messages;
mod state;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

use crate::app::App;
use crate::components::Command;

pub use lifecycle::restore_session_draft;
pub use state::{project_info_from_meta, session_info_from_meta};

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
            messages::save_draft(runtime, app, session_id, draft_text).await;
        }

        Command::SendMessage {
            workspace_id,
            session_id,
            content,
            attachments,
        } => {
            messages::send_message(runtime, app, workspace_id, session_id, content, attachments)
                .await;
        }

        Command::SendQueuedMessageNow {
            workspace_id,
            session_id,
            queue_index,
        } => {
            messages::send_queued_message_now(runtime, app, workspace_id, session_id, queue_index)
                .await;
        }

        Command::ApplyQueueAction(action) => {
            messages::apply_queue_action(runtime, app, action).await;
        }

        Command::DecidePermission {
            request_id,
            approved,
        } => {
            messages::decide_permission(runtime, app, request_id, approved).await;
        }

        Command::DecideTaskConfirmation { decision } => {
            messages::decide_task_confirmation(runtime, app, decision).await;
        }

        Command::CancelSession {
            workspace_id,
            session_id,
        } => {
            messages::cancel_session(runtime, app, workspace_id, session_id).await;
        }

        Command::RetryTask {
            workspace_id,
            session_id,
            task_id,
        } => {
            messages::retry_task(runtime, app, workspace_id, session_id, task_id).await;
        }

        Command::CancelTask {
            workspace_id,
            session_id,
            task_id,
        } => {
            messages::cancel_task(runtime, app, workspace_id, session_id, task_id).await;
        }

        Command::ClearSessionProjection => {
            messages::clear_session_projection(app);
        }

        Command::StartSession {
            workspace_id,
            model_profile,
        } => {
            lifecycle::start_session(runtime, app, workspace_id, model_profile).await;
        }

        Command::SwitchSession { session_id } => {
            lifecycle::switch_session(runtime, app, session_id).await;
        }

        Command::RenameSession { session_id, title } => {
            lifecycle::rename_session(runtime, app, session_id, title).await;
        }

        Command::ArchiveSession { session_id } => {
            lifecycle::archive_session(runtime, app, session_id).await;
        }

        Command::RestoreSession { session_id } => {
            lifecycle::restore_session(runtime, app, session_id).await;
        }

        Command::DeleteSession { session_id } => {
            lifecycle::delete_session(runtime, app, session_id).await;
        }

        Command::CreateProjectDraftSession { project_id } => {
            lifecycle::create_project_draft_session(runtime, app, project_id).await;
        }

        Command::CreateProjectWorktreeSession {
            project_id,
            branch_name,
        } => {
            lifecycle::create_project_worktree_session(runtime, app, project_id, branch_name).await;
        }

        _ => {}
    }
}

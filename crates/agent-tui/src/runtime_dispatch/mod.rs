mod mcp;
mod memory;
mod model;
mod monitor;
mod session;

use std::sync::Arc;

use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

use crate::app;
use crate::app::App;
use crate::components::Command;

pub use session::{project_info_from_meta, restore_session_draft, session_info_from_meta};

pub async fn dispatch_commands(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    commands: Vec<Command>,
) {
    for command in commands {
        match command {
            Command::SaveDraft { .. }
            | Command::SendMessage { .. }
            | Command::SendQueuedMessageNow { .. }
            | Command::ApplyQueueAction(_)
            | Command::DecidePermission { .. }
            | Command::CancelSession { .. }
            | Command::RetryTask { .. }
            | Command::CancelTask { .. }
            | Command::ClearSessionProjection
            | Command::StartSession { .. }
            | Command::SwitchSession { .. }
            | Command::RenameSession { .. }
            | Command::ArchiveSession { .. }
            | Command::RestoreSession { .. }
            | Command::DeleteSession { .. }
            | Command::CreateProjectDraftSession { .. }
            | Command::CreateProjectWorktreeSession { .. } => {
                session::dispatch(runtime, app, command).await;
            }

            Command::OpenMcpOverlay
            | Command::TrustMcpServer { .. }
            | Command::RevokeMcpTrust { .. }
            | Command::StartMcpServer { .. }
            | Command::StopMcpServer { .. }
            | Command::RefreshMcpTools { .. }
            | Command::CheckMcpHealth { .. }
            | Command::TestMcpConnectivity { .. }
            | Command::SetMcpToolDisabled { .. }
            | Command::ListMcpResources { .. }
            | Command::ListMcpPrompts { .. }
            | Command::ReadMcpResource { .. } => {
                mcp::dispatch(runtime, app, command).await;
            }

            Command::LoadMemories { .. } | Command::DeleteMemory { .. } => {
                memory::dispatch(runtime, app, command).await;
            }

            Command::OpenMonitorOverlay | Command::MonitorList | Command::MonitorStop { .. } => {
                monitor::dispatch(runtime, app, command).await;
            }

            Command::CompactSession { .. }
            | Command::SwitchModel { .. }
            | Command::OpenModelOverlay => {
                model::dispatch(runtime, app, command).await;
            }

            Command::SetSessionApprovalPolicy {
                workspace_id: _,
                session_id,
                approval,
            } => {
                if let Err(err) = runtime
                    .set_session_approval_policy(&session_id, approval)
                    .await
                {
                    app.status_bar
                        .push_notification(format!("[approval] error: {err}"));
                }
                app.sync_status_bar();
                app.state.render_scheduler.mark_dirty();
            }

            Command::SetSessionSandboxPolicy {
                workspace_id: _,
                session_id,
                sandbox,
            } => {
                if let Err(err) = runtime
                    .set_session_sandbox_policy(&session_id, &sandbox)
                    .await
                {
                    app.status_bar
                        .push_notification(format!("[sandbox] error: {err}"));
                }
                app.sync_status_bar();
                app.state.render_scheduler.mark_dirty();
            }

            Command::OpenSkillsOverlay
            | Command::OpenPluginsOverlay
            | Command::OpenHooksOverlay
            | Command::SaveHookSettings { .. }
            | Command::DeleteHookSettings { .. }
            | Command::OpenInstructionsOverlay
            | Command::OpenSystemPromptOverlay
            | Command::OpenAgentSettingsOverlay
            | Command::SaveAgentSettings { .. }
            | Command::DeleteAgentSettings { .. }
            | Command::CopyAgentSettings { .. }
            | Command::OpenConfigDir
            | Command::OpenAgentsDir
            | Command::OpenSkillsDir
            | Command::SaveProfileSettings { .. }
            | Command::SetProfileEnabled { .. }
            | Command::DeleteProfileSettings { .. }
            | Command::MoveProfileInOrder { .. }
            | Command::TestModelProfile { .. }
            | Command::TestModelProfileUrl { .. }
            | Command::OpenProfilesConfig
            | Command::SetSettingsConfigSource { .. }
            | Command::CycleSettingsProject { .. }
            | Command::SaveInstructions { .. }
            | Command::ListSkills
            | Command::ShowSkill { .. }
            | Command::ActivateSkill { .. }
            | Command::DeactivateSkill { .. }
            | Command::ListSkillCatalog { .. }
            | Command::InstallRemoteSkill { .. }
            | Command::InstallGithubSkill { .. }
            | Command::UpdateSkillSettings { .. }
            | Command::DeleteSkillSettings { .. }
            | Command::SetSkillEnabled { .. }
            | Command::SetSkillSourceEnabled { .. }
            | Command::AddSkillSource { .. }
            | Command::RemoveSkillSource { .. }
            | Command::RefreshSkillCatalog { .. }
            | Command::SearchRemoteSkills { .. }
            | Command::SetPluginEnabled { .. }
            | Command::DeletePluginSettings { .. }
            | Command::SetPluginMarketplaceSourceEnabled { .. }
            | Command::InstallPlugin { .. }
            | Command::SetMcpServerEnabled { .. }
            | Command::SaveMcpServerSettings { .. }
            | Command::DeleteMcpServerSettings { .. }
            | Command::OpenMcpConfig
            | Command::DisableMcpServerAtScope { .. }
            | Command::EnableMcpServerAtScope { .. }
            | Command::InstallMcpServer { .. }
            | Command::UninstallMcpServer { .. }
            | Command::SetMcpCatalogSourceEnabled { .. }
            | Command::AddMcpCatalogSource { .. }
            | Command::RemoveMcpCatalogSource { .. }
            | Command::CreateBlankProject { .. }
            | Command::AddExistingProject { .. }
            | Command::RenameProject { .. }
            | Command::RemoveProject { .. }
            | Command::MoveProject { .. }
            | Command::SetProjectExpanded { .. }
            | Command::RefreshProjectGitStatus { .. }
            | Command::InitProjectGit { .. }
            | Command::ShowProjectInstructions { .. }
            | Command::ExportTrace { .. }
            | Command::RefreshConfig => {
                dispatch_app_command(runtime, app, command).await;
            }
        }
    }
}

async fn dispatch_app_command(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    command: Command,
) {
    let refresh_mcp_after = matches!(
        command,
        Command::SetMcpServerEnabled { .. }
            | Command::SaveMcpServerSettings { .. }
            | Command::DeleteMcpServerSettings { .. }
            | Command::DisableMcpServerAtScope { .. }
            | Command::EnableMcpServerAtScope { .. }
            | Command::InstallMcpServer { .. }
            | Command::UninstallMcpServer { .. }
            | Command::SetMcpCatalogSourceEnabled { .. }
            | Command::AddMcpCatalogSource { .. }
            | Command::RemoveMcpCatalogSource { .. }
    );
    let refresh_model_after = matches!(
        command,
        Command::SaveProfileSettings { .. }
            | Command::SetProfileEnabled { .. }
            | Command::DeleteProfileSettings { .. }
            | Command::MoveProfileInOrder { .. }
    );

    app::dispatch_commands(runtime, app, vec![command]).await;

    if refresh_mcp_after && app.mcp_overlay.is_visible() {
        mcp::refresh_mcp_overlay(runtime, app).await;
    }
    if refresh_model_after && app.model_overlay.is_visible() {
        model::refresh_model_overlay(runtime, app).await;
    }
}

pub(crate) fn push_status_message(app: &mut App, content: String) {
    if content.trim().is_empty() {
        return;
    }
    app.state.push_status_message(content);
    if let Some(entry) = app.state.latest_status_message() {
        app.status_bar.push_notification(entry.message.clone());
    }
    app.state.render_scheduler.mark_dirty();
}

pub(crate) fn push_status_error(app: &mut App, content: String) {
    push_status_message(app, content);
}

mod agents;
mod common;
mod hooks;
mod instructions;
mod mcp;
mod models;
mod plugins;
mod projects;
mod skills;

use agent_core::facade::{
    HookSettingsInput, HooksSettingsView, InstructionsUpdateInput, McpFacade, PluginsFacade,
    ProjectFacade, SkillCatalogQuery, SkillInstallTarget,
};
use agent_core::{
    ActivateSkillRequest, AppFacade, DeactivateSkillRequest, ProjectGitStatus,
    ProjectGitStatusKind, ProjectInstructionSummary, ProjectMeta,
};

use super::App;
use crate::app_state::SettingsConfigSource;
use crate::components::{
    AgentOverlaySnapshot, Command, CommandPaletteSnapshot, CrossPanelEffect, McpOverlaySnapshot,
    McpServerEntry, ModelOverlaySnapshot, ModelProfileEntry, ModelProfileTestResult,
    PluginOverlaySnapshot, ProjectInfo, SkillEntry, SkillOverlaySnapshot,
};

pub use common::clear_session_projection;
pub use mcp::refresh_mcp_overlay;

pub async fn dispatch_commands<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
    commands: Vec<Command>,
) where
    F: AppFacade + ?Sized,
{
    for command in commands {
        match command {
            Command::SendMessage { .. }
            | Command::SaveDraft { .. }
            | Command::SendQueuedMessageNow { .. }
            | Command::ApplyQueueAction(_)
            | Command::DecidePermission { .. }
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
            | Command::ReadMcpResource { .. }
            | Command::CancelSession { .. }
            | Command::RetryTask { .. }
            | Command::CancelTask { .. }
            | Command::LoadMemories { .. }
            | Command::DeleteMemory { .. }
            | Command::StartSession { .. }
            | Command::SwitchSession { .. }
            | Command::RenameSession { .. }
            | Command::ArchiveSession { .. }
            | Command::RestoreSession { .. }
            | Command::DeleteSession { .. }
            | Command::CreateProjectDraftSession { .. }
            | Command::CreateProjectWorktreeSession { .. }
            | Command::CompactSession { .. }
            | Command::SwitchModel { .. }
            | Command::SetSessionApprovalPolicy { .. }
            | Command::SetSessionSandboxPolicy { .. } => {}

            Command::OpenMcpOverlay
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
            | Command::RemoveMcpCatalogSource { .. } => {
                mcp::dispatch(runtime, app, command).await;
            }

            Command::OpenModelOverlay
            | Command::SaveProfileSettings { .. }
            | Command::SetProfileEnabled { .. }
            | Command::DeleteProfileSettings { .. }
            | Command::MoveProfileInOrder { .. }
            | Command::TestModelProfile { .. }
            | Command::TestModelProfileUrl { .. }
            | Command::OpenProfilesConfig => {
                models::dispatch(runtime, app, command).await;
            }

            Command::OpenSkillsOverlay
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
            | Command::RefreshSkillCatalog { .. } => {
                skills::dispatch(runtime, app, command).await;
            }

            Command::OpenPluginsOverlay
            | Command::SetPluginEnabled { .. }
            | Command::DeletePluginSettings { .. }
            | Command::SetPluginMarketplaceSourceEnabled { .. }
            | Command::InstallPlugin { .. } => {
                plugins::dispatch(runtime, app, command).await;
            }

            Command::OpenHooksOverlay
            | Command::SaveHookSettings { .. }
            | Command::DeleteHookSettings { .. } => {
                hooks::dispatch(app, command);
            }

            Command::OpenInstructionsOverlay
            | Command::OpenSystemPromptOverlay
            | Command::SaveInstructions { .. } => {
                instructions::dispatch(app, command);
            }

            Command::OpenAgentSettingsOverlay
            | Command::SaveAgentSettings { .. }
            | Command::DeleteAgentSettings { .. }
            | Command::CopyAgentSettings { .. } => {
                agents::dispatch(runtime, app, command).await;
            }

            Command::CreateBlankProject { .. }
            | Command::AddExistingProject { .. }
            | Command::RenameProject { .. }
            | Command::RemoveProject { .. }
            | Command::MoveProject { .. }
            | Command::SetProjectExpanded { .. }
            | Command::RefreshProjectGitStatus { .. }
            | Command::InitProjectGit { .. }
            | Command::ShowProjectInstructions { .. } => {
                projects::dispatch(runtime, app, command).await;
            }

            Command::SetSettingsConfigSource { .. }
            | Command::CycleSettingsProject { .. }
            | Command::OpenConfigDir
            | Command::OpenAgentsDir
            | Command::OpenSkillsDir
            | Command::ClearSessionProjection => {
                common::dispatch(runtime, app, command).await;
            }
        }
    }
}

pub async fn refresh_command_palette<F>(runtime: &std::sync::Arc<F>, app: &mut App)
where
    F: AppFacade + ?Sized,
{
    let model_profiles = models::command_palette_model_profiles(runtime, app).await;
    let skills = skills::load_skill_entries(runtime, app)
        .await
        .unwrap_or_default();
    app.dispatch_effects(vec![
        CrossPanelEffect::UpdateCommandPalette(CommandPaletteSnapshot {
            model_profiles,
            skills,
        }),
        CrossPanelEffect::ShowCommandPalette,
    ]);
}

//! Application facade — the primary integration point for Kairox.
//!
//! All UIs (TUI, GUI) interact with the runtime through the [`AppFacade`] trait.
//! This trait provides a stable, object-safe interface for workspace management,
//! session lifecycle, messaging, permissions, and event streaming.

mod agents;
mod catalog;
mod mcp;
mod plugins;
mod project;
mod session;
mod settings;
mod skill_dtos;
mod skills;

pub use agents::AgentsFacade;
pub use catalog::{
    AddCatalogSourceRequest, CatalogQuery, CatalogSourceView, InstallOutcomeView, InstallRequest,
    InstalledEntry, ServerEntry,
};
pub use mcp::McpFacade;
pub use plugins::{
    InstallPluginRequest, PluginCatalogEntry, PluginComponentInventoryView, PluginDetailView,
    PluginInstallTarget, PluginMarketplaceSourceView, PluginSecurityMetadataView,
    PluginSettingsView, PluginsFacade,
};
pub use project::{
    ProjectFacade, ProjectGitStatus, ProjectGitStatusKind, ProjectInstructionSummary, ProjectMeta,
    ProjectSessionBinding, ProjectSessionVisibility,
};
pub use session::{
    AgentStatusInfo, AttachmentInfo, PermissionDecision, SendMessageRequest, SessionFacade,
    SessionMeta, StartSessionRequest, TaskGraphSnapshot, TaskSnapshot, TraceEntry, TraceExport,
    WorkspaceInfo,
};
pub use settings::{
    AgentSettingsInput, AgentSettingsScope, AgentSettingsView, EffectiveAgentView,
    EffectiveMcpServerView, EffectiveProfileView, HookSettingsInput, HookSettingsView,
    HookTemplateView, HooksSettingsView, InstructionsUpdateInput, InstructionsView,
    McpServerSettingsInput, McpServerSettingsTransport, McpServerSettingsView,
    ProfileSettingsInput, ProfileSettingsView,
};
pub use skill_dtos::{
    ActivateSkillRequest, ActiveSkillView, DeactivateSkillRequest, EffectiveSkillView,
    InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult,
    SkillCatalogEntry, SkillCatalogQuery, SkillDetail, SkillFieldMappingView, SkillInstallSource,
    SkillInstallTarget, SkillSettingsDetail, SkillSettingsScope, SkillSettingsView,
    SkillSourceView, SkillUpdateState, SkillView,
};
pub use skills::SkillsFacade;

use crate::{ProjectId, SessionId, TaskId, WorkspaceId};

/// AppFacade is the complete application facade, combining all sub-traits.
///
/// All UIs (TUI, GUI) interact with the runtime through this trait.
/// The canonical implementation is [`agent_runtime::LocalRuntime`],
/// but any mock or test implementation can substitute.
///
/// # Object Safety
///
/// This trait is object-safe and can be used as `dyn AppFacade`.
/// Every method has a default implementation that delegates to the
/// corresponding sub-trait, so implementors only need to implement
/// the sub-traits and write `impl AppFacade for T {}`.
#[async_trait::async_trait]
pub trait AppFacade:
    SessionFacade + SkillsFacade + McpFacade + ProjectFacade + AgentsFacade + PluginsFacade
{
    // ── Session ─────────────────────────────────────────────────────────

    async fn open_workspace(&self, path: String) -> crate::Result<WorkspaceInfo> {
        SessionFacade::open_workspace(self, path).await
    }
    async fn start_session(&self, request: StartSessionRequest) -> crate::Result<SessionId> {
        SessionFacade::start_session(self, request).await
    }
    async fn send_message(&self, request: SendMessageRequest) -> crate::Result<()> {
        SessionFacade::send_message(self, request).await
    }
    async fn decide_permission(&self, decision: PermissionDecision) -> crate::Result<()> {
        SessionFacade::decide_permission(self, decision).await
    }
    async fn cancel_session(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> crate::Result<()> {
        SessionFacade::cancel_session(self, workspace_id, session_id).await
    }
    async fn get_session_projection(
        &self,
        session_id: SessionId,
    ) -> crate::Result<crate::projection::SessionProjection> {
        SessionFacade::get_session_projection(self, session_id).await
    }
    async fn get_trace(&self, session_id: SessionId) -> crate::Result<Vec<TraceEntry>> {
        SessionFacade::get_trace(self, session_id).await
    }
    async fn export_trace(&self, session_id: SessionId) -> crate::Result<TraceExport> {
        SessionFacade::export_trace(self, session_id).await
    }
    fn subscribe_session(
        &self,
        session_id: SessionId,
    ) -> futures::stream::BoxStream<'static, crate::DomainEvent> {
        SessionFacade::subscribe_session(self, session_id)
    }
    fn subscribe_all(&self) -> futures::stream::BoxStream<'static, crate::DomainEvent> {
        SessionFacade::subscribe_all(self)
    }
    async fn list_workspaces(&self) -> crate::Result<Vec<WorkspaceInfo>> {
        SessionFacade::list_workspaces(self).await
    }
    async fn list_sessions(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<SessionMeta>> {
        SessionFacade::list_sessions(self, workspace_id).await
    }
    async fn rename_session(&self, session_id: &SessionId, title: String) -> crate::Result<()> {
        SessionFacade::rename_session(self, session_id, title).await
    }
    async fn soft_delete_session(&self, session_id: &SessionId) -> crate::Result<()> {
        SessionFacade::soft_delete_session(self, session_id).await
    }
    async fn permanently_delete_session(&self, session_id: &SessionId) -> crate::Result<()> {
        SessionFacade::permanently_delete_session(self, session_id).await
    }
    async fn restore_archived_session(&self, session_id: &SessionId) -> crate::Result<()> {
        SessionFacade::restore_archived_session(self, session_id).await
    }
    async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> crate::Result<usize> {
        SessionFacade::cleanup_expired_sessions(self, older_than).await
    }
    async fn get_task_graph(&self, session_id: SessionId) -> crate::Result<TaskGraphSnapshot> {
        SessionFacade::get_task_graph(self, session_id).await
    }
    async fn retry_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> crate::Result<()> {
        SessionFacade::retry_task(self, workspace_id, session_id, task_id).await
    }
    async fn cancel_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> crate::Result<()> {
        SessionFacade::cancel_task(self, workspace_id, session_id, task_id).await
    }
    async fn get_agent_status(&self, session_id: SessionId) -> crate::Result<Vec<AgentStatusInfo>> {
        SessionFacade::get_agent_status(self, session_id).await
    }
    async fn list_trajectories(
        &self,
        session_id: SessionId,
    ) -> crate::Result<Vec<crate::TrajectoryMeta>> {
        SessionFacade::list_trajectories(self, session_id).await
    }
    async fn get_trajectory_steps(
        &self,
        trajectory_id: crate::TrajectoryId,
    ) -> crate::Result<Vec<crate::TrajectoryStep>> {
        SessionFacade::get_trajectory_steps(self, trajectory_id).await
    }
    async fn export_trajectory(
        &self,
        trajectory_id: crate::TrajectoryId,
    ) -> crate::Result<serde_json::Value> {
        SessionFacade::export_trajectory(self, trajectory_id).await
    }

    // ── Skills ──────────────────────────────────────────────────────────

    async fn list_skills(&self) -> crate::Result<Vec<SkillView>> {
        SkillsFacade::list_skills(self).await
    }
    async fn get_skill(&self, skill_id: String) -> crate::Result<Option<SkillDetail>> {
        SkillsFacade::get_skill(self, skill_id).await
    }
    async fn activate_skill(
        &self,
        request: ActivateSkillRequest,
    ) -> crate::Result<ActiveSkillView> {
        SkillsFacade::activate_skill(self, request).await
    }
    async fn deactivate_skill(&self, request: DeactivateSkillRequest) -> crate::Result<()> {
        SkillsFacade::deactivate_skill(self, request).await
    }
    async fn list_active_skills(
        &self,
        session_id: SessionId,
    ) -> crate::Result<Vec<ActiveSkillView>> {
        SkillsFacade::list_active_skills(self, session_id).await
    }
    /// List all configured model profiles for settings UI.
    async fn list_profile_settings(
        &self,
        source_filter: Option<String>,
    ) -> crate::Result<Vec<ProfileSettingsView>> {
        McpFacade::list_profile_settings(self, source_filter).await
    }

    /// List model profiles with an explicit project root for project-scoped
    /// config lookups.
    async fn list_profile_settings_for_project(
        &self,
        source_filter: Option<String>,
        project_root: Option<String>,
    ) -> crate::Result<Vec<ProfileSettingsView>> {
        McpFacade::list_profile_settings_for_project(self, source_filter, project_root).await
    }

    /// Move a profile up or down in display order.
    async fn move_profile_in_order(&self, alias: String, direction: i32) -> crate::Result<()> {
        McpFacade::move_profile_in_order(self, alias, direction).await
    }

    /// Open the config directory in the system file manager.
    async fn open_config_dir(&self) -> crate::Result<Option<String>> {
        McpFacade::open_config_dir(self).await
    }

    /// Open the profiles.toml config file with the system default text editor.
    async fn open_profiles_config_file(&self) -> crate::Result<Option<String>> {
        McpFacade::open_profiles_config_file(self).await
    }

    /// List skills for settings UI.
    async fn list_skill_settings(&self) -> crate::Result<Vec<SkillSettingsView>> {
        SkillsFacade::list_skill_settings(self).await
    }
    async fn get_skill_settings_detail(
        &self,
        skill_id: String,
    ) -> crate::Result<Option<SkillSettingsDetail>> {
        SkillsFacade::get_skill_settings_detail(self, skill_id).await
    }
    async fn set_skill_enabled(&self, skill_id: String, enabled: bool) -> crate::Result<()> {
        SkillsFacade::set_skill_enabled(self, skill_id, enabled).await
    }
    async fn delete_skill_settings(&self, skill_id: String) -> crate::Result<()> {
        SkillsFacade::delete_skill_settings(self, skill_id).await
    }
    async fn search_remote_skills(
        &self,
        query: String,
    ) -> crate::Result<Vec<RemoteSkillSearchResult>> {
        SkillsFacade::search_remote_skills(self, query).await
    }
    async fn install_remote_skill(
        &self,
        request: InstallRemoteSkillRequest,
    ) -> crate::Result<SkillSettingsView> {
        SkillsFacade::install_remote_skill(self, request).await
    }
    async fn install_github_skill(
        &self,
        request: InstallGithubSkillRequest,
    ) -> crate::Result<SkillSettingsView> {
        SkillsFacade::install_github_skill(self, request).await
    }
    async fn update_skill(&self, skill_id: String) -> crate::Result<SkillSettingsView> {
        SkillsFacade::update_skill(self, skill_id).await
    }
    async fn list_skill_catalog(
        &self,
        query: SkillCatalogQuery,
    ) -> crate::Result<Vec<SkillCatalogEntry>> {
        SkillsFacade::list_skill_catalog(self, query).await
    }
    async fn list_skill_sources(&self) -> crate::Result<Vec<SkillSourceView>> {
        SkillsFacade::list_skill_sources(self).await
    }
    async fn add_skill_source(&self, config: SkillSourceView) -> crate::Result<()> {
        SkillsFacade::add_skill_source(self, config).await
    }
    async fn remove_skill_source(&self, id: String) -> crate::Result<()> {
        SkillsFacade::remove_skill_source(self, id).await
    }
    async fn set_skill_source_enabled(&self, id: String, enabled: bool) -> crate::Result<()> {
        SkillsFacade::set_skill_source_enabled(self, id, enabled).await
    }
    async fn refresh_skill_catalog(&self) -> crate::Result<()> {
        SkillsFacade::refresh_skill_catalog(self).await
    }
    async fn open_skills_dir(&self) -> crate::Result<Option<String>> {
        SkillsFacade::open_skills_dir(self).await
    }

    // ── Agents ─────────────────────────────────────────────────────────

    async fn list_agent_settings(&self) -> crate::Result<Vec<AgentSettingsView>> {
        AgentsFacade::list_agent_settings(self).await
    }

    async fn upsert_agent_settings(
        &self,
        input: AgentSettingsInput,
    ) -> crate::Result<AgentSettingsView> {
        AgentsFacade::upsert_agent_settings(self, input).await
    }

    async fn delete_agent_settings(&self, agent_id: String) -> crate::Result<()> {
        AgentsFacade::delete_agent_settings(self, agent_id).await
    }

    async fn copy_agent_settings(
        &self,
        agent_id: String,
        scope: AgentSettingsScope,
    ) -> crate::Result<AgentSettingsView> {
        AgentsFacade::copy_agent_settings(self, agent_id, scope).await
    }

    async fn open_agents_dir(&self) -> crate::Result<Option<String>> {
        AgentsFacade::open_agents_dir(self).await
    }

    // ── MCP / Marketplace / Profile ─────────────────────────────────────

    async fn list_mcp_server_settings(
        &self,
        source_filter: Option<String>,
    ) -> crate::Result<Vec<McpServerSettingsView>> {
        McpFacade::list_mcp_server_settings(self, source_filter).await
    }

    /// List MCP servers with an explicit project root for project-scoped
    /// config lookups.
    async fn list_mcp_server_settings_for_project(
        &self,
        source_filter: Option<String>,
        project_root: Option<String>,
    ) -> crate::Result<Vec<McpServerSettingsView>> {
        McpFacade::list_mcp_server_settings_for_project(self, source_filter, project_root).await
    }
    async fn upsert_mcp_server_settings(
        &self,
        input: McpServerSettingsInput,
    ) -> crate::Result<McpServerSettingsView> {
        McpFacade::upsert_mcp_server_settings(self, input).await
    }
    async fn delete_mcp_server_settings(&self, server_id: String) -> crate::Result<()> {
        McpFacade::delete_mcp_server_settings(self, server_id).await
    }
    async fn set_mcp_server_enabled(&self, server_id: String, enabled: bool) -> crate::Result<()> {
        McpFacade::set_mcp_server_enabled(self, server_id, enabled).await
    }
    async fn open_mcp_config_file(&self) -> crate::Result<Option<String>> {
        McpFacade::open_mcp_config_file(self).await
    }
    async fn upsert_profile_settings(
        &self,
        input: ProfileSettingsInput,
    ) -> crate::Result<ProfileSettingsView> {
        McpFacade::upsert_profile_settings(self, input).await
    }
    async fn set_profile_enabled(&self, alias: String, enabled: bool) -> crate::Result<()> {
        McpFacade::set_profile_enabled(self, alias, enabled).await
    }
    async fn delete_profile_settings(&self, alias: String) -> crate::Result<()> {
        McpFacade::delete_profile_settings(self, alias).await
    }
    async fn list_catalog(&self, query: CatalogQuery) -> crate::Result<Vec<ServerEntry>> {
        McpFacade::list_catalog(self, query).await
    }
    async fn get_catalog_entry(
        &self,
        id: String,
        source: Option<String>,
    ) -> crate::Result<Option<ServerEntry>> {
        McpFacade::get_catalog_entry(self, id, source).await
    }
    async fn refresh_catalog(&self, source: Option<String>) -> crate::Result<()> {
        McpFacade::refresh_catalog(self, source).await
    }
    async fn install_catalog_entry(
        &self,
        request: InstallRequest,
    ) -> crate::Result<InstallOutcomeView> {
        McpFacade::install_catalog_entry(self, request).await
    }
    async fn uninstall_catalog_entry(&self, server_id: String) -> crate::Result<()> {
        McpFacade::uninstall_catalog_entry(self, server_id).await
    }
    async fn list_installed_entries(&self) -> crate::Result<Vec<InstalledEntry>> {
        McpFacade::list_installed_entries(self).await
    }
    async fn list_catalog_sources(&self) -> crate::Result<Vec<CatalogSourceView>> {
        McpFacade::list_catalog_sources(self).await
    }
    async fn add_catalog_source(&self, request: AddCatalogSourceRequest) -> crate::Result<()> {
        McpFacade::add_catalog_source(self, request).await
    }
    async fn remove_catalog_source(&self, id: String) -> crate::Result<()> {
        McpFacade::remove_catalog_source(self, id).await
    }
    async fn set_catalog_source_enabled(&self, id: String, enabled: bool) -> crate::Result<()> {
        McpFacade::set_catalog_source_enabled(self, id, enabled).await
    }

    // ── Projects ────────────────────────────────────────────────────────

    async fn list_projects(&self, workspace_id: &WorkspaceId) -> crate::Result<Vec<ProjectMeta>> {
        ProjectFacade::list_projects(self, workspace_id).await
    }
    async fn create_blank_project(
        &self,
        workspace_id: WorkspaceId,
        display_name: Option<String>,
    ) -> crate::Result<ProjectMeta> {
        ProjectFacade::create_blank_project(self, workspace_id, display_name).await
    }
    async fn add_existing_project(
        &self,
        workspace_id: WorkspaceId,
        path: String,
    ) -> crate::Result<ProjectMeta> {
        ProjectFacade::add_existing_project(self, workspace_id, path).await
    }
    async fn rename_project(
        &self,
        project_id: ProjectId,
        display_name: String,
    ) -> crate::Result<()> {
        ProjectFacade::rename_project(self, project_id, display_name).await
    }
    async fn remove_project(&self, project_id: ProjectId) -> crate::Result<()> {
        ProjectFacade::remove_project(self, project_id).await
    }
    async fn restore_project_session(&self, session_id: SessionId) -> crate::Result<ProjectMeta> {
        ProjectFacade::restore_project_session(self, session_id).await
    }
    async fn update_project_order(&self, project_ids: Vec<ProjectId>) -> crate::Result<()> {
        ProjectFacade::update_project_order(self, project_ids).await
    }
    async fn update_project_expanded(
        &self,
        project_id: ProjectId,
        expanded: bool,
    ) -> crate::Result<()> {
        ProjectFacade::update_project_expanded(self, project_id, expanded).await
    }
    async fn create_project_draft_session(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<SessionId> {
        ProjectFacade::create_project_draft_session(self, project_id).await
    }
    async fn create_project_worktree_session(
        &self,
        project_id: ProjectId,
        branch_name: String,
    ) -> crate::Result<SessionId> {
        ProjectFacade::create_project_worktree_session(self, project_id, branch_name).await
    }
    async fn list_project_branches(&self, project_id: ProjectId) -> crate::Result<Vec<String>> {
        ProjectFacade::list_project_branches(self, project_id).await
    }
    async fn list_project_sessions(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<Vec<SessionMeta>> {
        ProjectFacade::list_project_sessions(self, project_id).await
    }
    async fn list_archived_sessions(
        &self,
        workspace_id: &WorkspaceId,
    ) -> crate::Result<Vec<SessionMeta>> {
        ProjectFacade::list_archived_sessions(self, workspace_id).await
    }
    async fn get_project_git_status(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<ProjectGitStatus> {
        ProjectFacade::get_project_git_status(self, project_id).await
    }
    async fn get_session_git_status(
        &self,
        session_id: SessionId,
    ) -> crate::Result<ProjectGitStatus> {
        ProjectFacade::get_session_git_status(self, session_id).await
    }
    async fn init_project_git(&self, project_id: ProjectId) -> crate::Result<ProjectGitStatus> {
        ProjectFacade::init_project_git(self, project_id).await
    }
    async fn get_project_instruction_summary(
        &self,
        project_id: ProjectId,
    ) -> crate::Result<ProjectInstructionSummary> {
        ProjectFacade::get_project_instruction_summary(self, project_id).await
    }
}

#[cfg(test)]
#[path = "facade_tests.rs"]
mod tests;

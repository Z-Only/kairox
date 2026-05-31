use agent_core::{AttachmentInfo, ProjectId, SessionId};

use super::QueueAction;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Command {
    SendMessage {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
        content: String,
        attachments: Vec<AttachmentInfo>,
    },
    SaveDraft {
        session_id: SessionId,
        draft_text: String,
    },
    SendQueuedMessageNow {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
        queue_index: usize,
    },
    ApplyQueueAction(QueueAction),
    /// Clear only the local projection for the current session.
    ClearSessionProjection,
    DecidePermission {
        request_id: String,
        approved: bool,
    },
    /// Trust an MCP server so future tool calls from it are auto-approved.
    TrustMcpServer {
        server_id: String,
    },
    /// Revoke stored trust for an MCP server.
    RevokeMcpTrust {
        server_id: String,
    },
    /// Build an MCP server snapshot and open the overlay.
    OpenMcpOverlay,
    /// Start a stopped/failed MCP server from the overlay.
    StartMcpServer {
        server_id: String,
    },
    /// Stop a running MCP server from the overlay.
    StopMcpServer {
        server_id: String,
    },
    /// Refresh the cached tool list from a running MCP server.
    RefreshMcpTools {
        server_id: String,
    },
    /// Run the MCP health check and populate the tools tab.
    CheckMcpHealth {
        server_id: String,
    },
    /// Run a connectivity probe against an MCP server.
    TestMcpConnectivity {
        server_id: String,
    },
    /// Enable or disable one runtime-discovered MCP tool.
    SetMcpToolDisabled {
        server_id: String,
        tool_name: String,
        disabled: bool,
    },
    /// List MCP resources for a running server.
    ListMcpResources {
        server_id: String,
    },
    /// List MCP prompts for a running server.
    ListMcpPrompts {
        server_id: String,
    },
    /// Read one MCP resource.
    ReadMcpResource {
        server_id: String,
        uri: String,
    },
    /// Enable or disable one MCP server in writable settings.
    SetMcpServerEnabled {
        server_id: String,
        enabled: bool,
    },
    /// Save one MCP server setting into the writable MCP config.
    SaveMcpServerSettings {
        input: agent_core::facade::McpServerSettingsInput,
    },
    /// Delete one writable MCP server setting.
    DeleteMcpServerSettings {
        server_id: String,
    },
    /// Open the writable MCP config file.
    OpenMcpConfig,
    /// Disable an inherited MCP server at project scope.
    DisableMcpServerAtScope {
        server_id: String,
    },
    /// Re-enable an inherited MCP server at project scope.
    EnableMcpServerAtScope {
        server_id: String,
    },
    /// Install one MCP catalog entry.
    InstallMcpServer {
        request: agent_core::facade::InstallRequest,
    },
    /// Uninstall one installed MCP server.
    UninstallMcpServer {
        server_id: String,
    },
    /// Enable or disable one MCP catalog source.
    SetMcpCatalogSourceEnabled {
        source_id: String,
        enabled: bool,
    },
    /// Add one MCP catalog source.
    AddMcpCatalogSource {
        request: agent_core::facade::AddCatalogSourceRequest,
    },
    /// Remove one MCP catalog source.
    RemoveMcpCatalogSource {
        source_id: String,
    },
    CancelSession {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
    },
    RetryTask {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
        task_id: agent_core::TaskId,
    },
    CancelTask {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
        task_id: agent_core::TaskId,
    },
    LoadMemories {
        scope: Option<agent_memory::MemoryScope>,
        keywords: Vec<String>,
        limit: usize,
    },
    DeleteMemory {
        memory_id: String,
    },
    StartSession {
        workspace_id: agent_core::WorkspaceId,
        model_profile: String,
    },
    SwitchSession {
        session_id: SessionId,
    },
    RenameSession {
        session_id: SessionId,
        title: String,
    },
    ArchiveSession {
        session_id: SessionId,
    },
    RestoreSession {
        session_id: SessionId,
    },
    DeleteSession {
        session_id: SessionId,
    },
    CreateBlankProject {
        display_name: Option<String>,
    },
    AddExistingProject {
        path: String,
    },
    RenameProject {
        project_id: ProjectId,
        display_name: String,
    },
    RemoveProject {
        project_id: ProjectId,
    },
    MoveProject {
        project_id: ProjectId,
        direction: i32,
    },
    SetProjectExpanded {
        project_id: ProjectId,
        expanded: bool,
    },
    RefreshProjectGitStatus {
        project_id: ProjectId,
    },
    InitProjectGit {
        project_id: ProjectId,
    },
    ShowProjectInstructions {
        project_id: ProjectId,
    },
    CreateProjectDraftSession {
        project_id: ProjectId,
    },
    CreateProjectWorktreeSession {
        project_id: ProjectId,
        branch_name: String,
    },
    /// P3: user typed `:compact` in the chat panel; ask the runtime to
    /// summarise older history into a compaction summary.
    CompactSession {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
    },
    /// P4: user typed `:model <alias>` in the chat panel, or selected a
    /// profile from the model overlay; ask the runtime to switch the active
    /// model profile mid-session. `workspace_id` is carried for symmetry
    /// with sibling variants. When `reasoning_effort` is `None` the runtime
    /// keeps the existing effort (or default) for reasoning models; the
    /// `:model <alias>` parser always sends `None`, while the overlay's
    /// effort picker populates it.
    SwitchModel {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
        alias: String,
        reasoning_effort: Option<String>,
    },
    /// Build a skill snapshot and open the skills overlay (Ctrl+S or
    /// `:skills` typed in the chat panel).
    OpenSkillsOverlay,
    /// Build a model-profile snapshot and open the model overlay.
    OpenModelOverlay,
    /// Enable or disable one writable model profile setting.
    SetProfileEnabled {
        alias: String,
        enabled: bool,
    },
    /// Save one writable model profile setting.
    SaveProfileSettings {
        input: agent_core::facade::ProfileSettingsInput,
    },
    /// Delete one writable model profile setting.
    DeleteProfileSettings {
        alias: String,
    },
    /// Move a profile up or down in display order.
    MoveProfileInOrder {
        alias: String,
        direction: i32,
    },
    /// Run a lightweight connectivity check for one model profile.
    TestModelProfile {
        alias: String,
    },
    /// Run a lightweight connectivity check for an unsaved model profile base URL.
    TestModelProfileUrl {
        alias: String,
        base_url: String,
    },
    /// Open the writable Kairox config directory.
    OpenConfigDir,
    /// Open the writable profiles config file.
    OpenProfilesConfig,
    /// Select whether settings overlays read/write user or project config.
    SetSettingsConfigSource {
        source: crate::app_state::SettingsConfigSource,
    },
    /// Cycle the project used by project-scoped settings overlays.
    CycleSettingsProject {
        direction: i32,
    },
    /// Build an agent settings snapshot and open the agent manager overlay.
    OpenAgentSettingsOverlay,
    /// Save a user/project agent settings profile.
    SaveAgentSettings {
        input: agent_core::facade::AgentSettingsInput,
    },
    /// Delete one editable agent settings profile.
    DeleteAgentSettings {
        settings_id: String,
    },
    /// Copy one agent settings profile to a writable scope.
    CopyAgentSettings {
        settings_id: String,
        scope: agent_core::facade::AgentSettingsScope,
    },
    /// Open the writable user agents directory.
    OpenAgentsDir,
    /// Open the writable user skills directory.
    OpenSkillsDir,
    /// Build a plugin manager snapshot and open the plugin overlay.
    OpenPluginsOverlay,
    /// Build a hooks settings snapshot and open the hooks overlay.
    OpenHooksOverlay,
    /// Save one user/project hook setting.
    SaveHookSettings {
        input: agent_core::facade::HookSettingsInput,
    },
    /// Delete one user/project hook setting.
    DeleteHookSettings {
        scope: agent_core::ConfigScope,
        event: String,
        id: String,
    },
    /// Build an instructions snapshot and open the instructions settings overlay.
    OpenInstructionsOverlay,
    /// Build an instructions snapshot and show the system prompt read-only tab.
    OpenSystemPromptOverlay,
    /// Save user/project instructions from the instructions overlay.
    SaveInstructions {
        scope: agent_core::ConfigScope,
        text: String,
    },
    /// User typed `:skills` to list discovered native skills.
    ListSkills,
    /// User typed `:skill show <id>` to show one native skill.
    ShowSkill {
        skill_id: String,
    },
    /// User typed `:skill activate <id>` to activate one skill for the current session.
    ActivateSkill {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
        skill_id: String,
    },
    /// User typed `:skill deactivate <id>` to deactivate one skill for the current session.
    DeactivateSkill {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
        skill_id: String,
    },
    /// Search/list skills from the configured skill catalog.
    ListSkillCatalog {
        keyword: Option<String>,
        sources: Option<Vec<String>>,
    },
    /// Install one remote catalog skill into the selected target.
    InstallRemoteSkill {
        request: agent_core::facade::InstallRemoteSkillRequest,
    },
    /// Install one GitHub skill into user settings.
    InstallGithubSkill {
        request: agent_core::facade::InstallGithubSkillRequest,
    },
    /// Update one installed skill.
    UpdateSkillSettings {
        skill_id: String,
    },
    /// Delete one installed skill configuration.
    DeleteSkillSettings {
        skill_id: String,
    },
    /// Enable or disable one installed skill setting.
    SetSkillEnabled {
        skill_id: String,
        enabled: bool,
    },
    /// Enable or disable one skill catalog source.
    SetSkillSourceEnabled {
        source_id: String,
        enabled: bool,
    },
    /// Add one skill catalog source.
    AddSkillSource {
        config: agent_core::facade::SkillSourceView,
    },
    /// Remove one skill catalog source.
    RemoveSkillSource {
        source_id: String,
    },
    /// Refresh the configured skill catalog provider cache.
    RefreshSkillCatalog {
        keyword: Option<String>,
        sources: Option<Vec<String>>,
    },
    /// Enable or disable one installed plugin.
    SetPluginEnabled {
        settings_id: String,
        enabled: bool,
    },
    /// Delete one installed plugin configuration.
    DeletePluginSettings {
        settings_id: String,
    },
    /// Enable or disable one plugin marketplace source.
    SetPluginMarketplaceSourceEnabled {
        source_id: String,
        enabled: bool,
    },
    /// Install one catalog plugin into the selected target.
    InstallPlugin {
        request: agent_core::facade::InstallPluginRequest,
    },
    /// User cycled the approval-axis policy (Shift+A while not focused on Sessions).
    /// The runtime should apply it to the current session, if any.
    SetSessionApprovalPolicy {
        workspace_id: agent_core::WorkspaceId,
        session_id: agent_core::SessionId,
        approval: agent_tools::ApprovalPolicy,
    },
    /// User cycled the sandbox-axis policy (Shift+B).
    /// The runtime should apply it to the current session, if any.
    SetSessionSandboxPolicy {
        workspace_id: agent_core::WorkspaceId,
        session_id: agent_core::SessionId,
        sandbox: agent_tools::SandboxPolicy,
    },
    /// List all active monitors in the current workspace.
    MonitorList,
    /// Stop a specific monitor by ID.
    MonitorStop {
        monitor_id: String,
    },
    /// Export the current session's trace to a JSON file for diagnostics/replay.
    ExportTrace {
        session_id: SessionId,
    },
    /// Reload configuration from disk (user + project TOML files).
    RefreshConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestructiveConfirmationTarget {
    scope: &'static str,
    id: String,
    description: String,
}

impl DestructiveConfirmationTarget {
    pub(crate) fn new(
        scope: &'static str,
        id: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            scope,
            id: id.into(),
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DestructiveConfirmationState {
    pending: Option<DestructiveConfirmationTarget>,
}

impl DestructiveConfirmationState {
    pub fn clear(&mut self) {
        self.pending = None;
    }

    pub fn arm_or_confirm(&mut self, target: DestructiveConfirmationTarget) -> bool {
        if self.pending.as_ref() == Some(&target) {
            self.pending = None;
            true
        } else {
            self.pending = Some(target);
            false
        }
    }

    pub fn pending_hint(&self) -> Option<String> {
        self.pending.as_ref().map(|target| {
            format!(
                "Press the destructive shortcut again to {}",
                target.description
            )
        })
    }
}

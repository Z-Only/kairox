pub mod agent_overlay;
pub mod chat;
pub mod command_palette;
pub mod help_overlay;
pub mod hooks_overlay;
pub mod instructions_overlay;
pub mod mcp_overlay;
pub mod model_overlay;
pub mod permission_modal;
pub mod plugin_overlay;
pub mod sessions;
pub mod skills_overlay;
pub mod status_bar;
pub mod trace;

use agent_core::{
    AttachmentInfo, ProjectGitStatus, ProjectId, ProjectInstructionSummary,
    ProjectSessionVisibility, SessionId,
};
use ratatui::layout::Rect;
use ratatui::Frame;

/// A self-contained UI panel that handles events and renders itself.
///
/// Components never directly reference other components.
/// Cross-panel communication flows exclusively through `CrossPanelEffect`
/// routed by the App layer.
#[allow(unused_variables)]
pub trait Component {
    /// Process an incoming event. Returns (cross-panel effects, runtime commands).
    #[allow(dead_code)]
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>);

    /// Receive a cross-panel effect dispatched by the App layer.
    fn handle_effect(&mut self, effect: &CrossPanelEffect);

    /// Render this component into the given area.
    fn render(&self, area: Rect, frame: &mut Frame);

    /// Whether this component currently holds focus.
    fn focused(&self) -> bool;

    /// Set focus state (for highlight rendering).
    fn set_focused(&mut self, focused: bool);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Chat,
    Sessions,
    Trace,
    PermissionModal,
    McpOverlay,
    CommandPalette,
    SkillsOverlay,
    ModelOverlay,
    AgentOverlay,
    PluginOverlay,
    HooksOverlay,
    InstructionsOverlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpOverlaySnapshot {
    pub focus: FocusTarget,
}

/// User-facing status of an MCP server, mirrored from `agent_mcp::types::McpServerStatus`.
/// Kept local to the TUI so the overlay can be tested without spinning up a real manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpServerStatusView {
    Stopped,
    Starting,
    Running,
    Failed,
}

/// Snapshot row used to populate the MCP overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerEntry {
    pub server_id: String,
    pub status: McpServerStatusView,
    pub trusted: bool,
    pub tool_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolEntry {
    pub server_id: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<String>,
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpResourceEntry {
    pub server_id: String,
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpPromptEntry {
    pub server_id: String,
    pub name: String,
    pub description: Option<String>,
    pub argument_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpConnectivityEntry {
    pub server_id: String,
    pub connected: bool,
    pub tool_count: Option<u32>,
    pub reason: Option<String>,
}

/// Snapshot payload for opening the MCP overlay. Runtime server state is
/// supplied by the TUI main loop, while settings/catalog data comes from the
/// MCP facade so tests can exercise the command path with fake facades.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpOverlaySnapshot {
    pub runtime_servers: Vec<McpServerEntry>,
    pub settings: Vec<agent_core::facade::McpServerSettingsView>,
    pub installed: Vec<agent_core::facade::InstalledEntry>,
    pub catalog: Vec<agent_core::facade::ServerEntry>,
    pub sources: Vec<agent_core::facade::CatalogSourceView>,
}

/// Snapshot row used to populate the skills overlay. Built from
/// `SkillView` + the per-session active list before opening the overlay,
/// kept local to the TUI so the overlay can be tested without spinning
/// up a real `SkillRegistry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub activation_mode: String,
    pub active: bool,
}

/// Snapshot payload for opening the skills manager overlay.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillOverlaySnapshot {
    pub discovered: Vec<SkillEntry>,
    pub installed: Vec<agent_core::facade::SkillSettingsView>,
    pub catalog: Vec<agent_core::facade::SkillCatalogEntry>,
    pub sources: Vec<agent_core::facade::SkillSourceView>,
    pub install_target: agent_core::facade::SkillInstallTarget,
}

impl From<Vec<SkillEntry>> for SkillOverlaySnapshot {
    fn from(discovered: Vec<SkillEntry>) -> Self {
        Self {
            discovered,
            installed: Vec::new(),
            catalog: Vec::new(),
            sources: Vec::new(),
            install_target: agent_core::facade::SkillInstallTarget::User,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandPaletteSnapshot {
    pub model_profiles: Vec<ModelProfileEntry>,
    pub skills: Vec<SkillEntry>,
}

/// Snapshot row used to populate the model profile selector overlay.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelProfileEntry {
    pub alias: String,
    pub provider_display: String,
    pub model_display: String,
    pub context_window: Option<u64>,
    pub output_limit: Option<u64>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub max_tokens: Option<u64>,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub supports_reasoning: bool,
    pub enabled: bool,
    pub writable: bool,
    pub source: String,
    pub has_api_key: bool,
}

/// Snapshot payload for opening the model overlay. `current_alias`/`current_effort`
/// reflect the active session at snapshot time so the overlay can highlight them.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelOverlaySnapshot {
    pub profiles: Vec<ModelProfileEntry>,
    pub current_alias: Option<String>,
    pub current_effort: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProfileTestResult {
    pub alias: String,
    pub ok: bool,
    pub message: Option<String>,
}

/// Snapshot payload for opening the agent settings overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentOverlaySnapshot {
    pub agents: Vec<agent_core::facade::AgentSettingsView>,
}

/// Snapshot payload for opening the plugin manager overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginOverlaySnapshot {
    pub plugins: Vec<agent_core::facade::PluginSettingsView>,
    pub catalog: Vec<agent_core::facade::PluginCatalogEntry>,
    pub sources: Vec<agent_core::facade::PluginMarketplaceSourceView>,
    pub install_target: agent_core::facade::PluginInstallTarget,
}

/// Catalog filters owned by the plugin overlay and applied by the app layer
/// when refreshing marketplace results.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginCatalogFilters {
    pub marketplace_id: Option<String>,
    pub keyword: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum RiskLevel {
    Write,
    Destructive,
    /// MCP tool invocation — external server tool
    McpTool {
        server_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    pub request_id: String,
    pub tool_id: String,
    pub tool_preview: String,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SessionState {
    Active,
    Idle,
    Error(String),
    AwaitingPermission,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInfo {
    pub id: ProjectId,
    pub display_name: String,
    pub root_path: String,
    pub expanded: bool,
    pub git_status: Option<ProjectGitStatus>,
    pub instruction_summary: Option<ProjectInstructionSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: SessionId,
    pub title: String,
    pub model_profile: String,
    pub state: SessionState,
    pub pinned: bool,
    pub archived: bool,
    pub project_id: Option<ProjectId>,
    pub worktree_path: Option<String>,
    pub branch: Option<String>,
    pub visibility: Option<ProjectSessionVisibility>,
}

/// A message typed while the session is busy and held until the session
/// returns to idle. Mirrors the GUI `QueuedMessage` contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedMessage {
    pub content: String,
    pub attachments: Vec<AttachmentInfo>,
}

/// Local TUI composer actions for queued messages. These never change runtime
/// queue semantics; they only mutate or dispatch messages already held by the
/// TUI chat panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueAction {
    View,
    SelectPrevious,
    SelectNext,
    MoveSelectedUp,
    MoveSelectedDown,
    RestoreSelectedForEdit,
    DeleteSelected,
    SendSelectedNow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusInfo {
    pub profile: String,
    pub permission_mode: String,
    pub session_count: usize,
    pub mcp_server_count: usize,
    pub session_metadata: Vec<String>,
    pub hint: String,
    pub error: Option<String>,
    /// P3: latest `ContextAssembled.usage`. `None` until the first event.
    pub context_usage: Option<agent_core::context_types::ContextUsage>,
    /// P3: `true` between `ContextCompactionStarted` and `Completed`/`Failed`.
    pub compacting: bool,
}

pub fn compact_worktree_path(path: &str) -> String {
    path.split_once(".kairox/")
        .map(|(_, suffix)| suffix.to_string())
        .unwrap_or_else(|| path.to_string())
}

pub fn project_instruction_source_label(summary: &ProjectInstructionSummary) -> Option<String> {
    if summary.source_paths.is_empty() {
        return None;
    }

    let names = summary
        .source_paths
        .iter()
        .filter_map(|path| std::path::Path::new(path).file_name())
        .filter_map(std::ffi::OsStr::to_str)
        .take(2)
        .collect::<Vec<_>>();

    if names.is_empty() {
        Some(summary.source_paths.len().to_string())
    } else {
        Some(names.join(", "))
    }
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum CrossPanelEffect {
    SwitchFocus(FocusTarget),
    ShowPermissionPrompt(PermissionRequest),
    ResolvePermissionPrompt {
        request_id: String,
        approved: bool,
    },
    DismissPermissionPrompt,
    UpdateSessionList(Vec<SessionInfo>),
    SetStatus(StatusInfo),
    NavigateToSession(SessionId),
    StartStreaming,
    StopStreaming,
    ShowMcpOverlay(McpOverlaySnapshot),
    DismissMcpOverlay,
    McpToolsLoaded {
        server_id: String,
        tools: Vec<McpToolEntry>,
        healthy: bool,
        error: Option<String>,
    },
    McpConnectivityChecked(McpConnectivityEntry),
    McpResourcesLoaded {
        server_id: String,
        resources: Vec<McpResourceEntry>,
    },
    McpPromptsLoaded {
        server_id: String,
        prompts: Vec<McpPromptEntry>,
    },
    McpResourceRead {
        server_id: String,
        uri: String,
        preview: String,
    },
    /// Open the command palette overlay.
    ShowCommandPalette,
    /// Refresh command palette dynamic rows before opening or while visible.
    UpdateCommandPalette(CommandPaletteSnapshot),
    /// Close the command palette overlay.
    DismissCommandPalette,
    /// Insert the given text at the start of the chat input and place the
    /// cursor at the end. Used by the command palette to hand back a slash
    /// prefix that needs an argument.
    PrefillChatInput(String),
    /// Build/refresh the skills overlay snapshot and open it.
    ShowSkillsOverlay(SkillOverlaySnapshot),
    /// Close the skills overlay.
    DismissSkillsOverlay,
    /// Deliver a skill's rendered markdown body to the open overlay so it
    /// can switch to inline detail view.
    ShowSkillBody {
        skill_id: String,
        body: String,
    },
    ShowModelOverlay(ModelOverlaySnapshot),
    ModelProfileTested(ModelProfileTestResult),
    DismissModelOverlay,
    ShowAgentSettingsOverlay(AgentOverlaySnapshot),
    DismissAgentSettingsOverlay,
    ShowPluginsOverlay(PluginOverlaySnapshot),
    DismissPluginsOverlay,
    ShowHooksOverlay(agent_core::facade::HooksSettingsView),
    DismissHooksOverlay,
    ShowInstructionsOverlay(agent_core::facade::InstructionsView),
    ShowSystemPromptOverlay(agent_core::facade::InstructionsView),
    DismissInstructionsOverlay,
    ShowHelpOverlay(HelpOverlaySnapshot),
    DismissHelpOverlay,
}

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
    /// User cycled the permission mode from the status bar (Shift+P).
    /// The runtime should apply the new mode for subsequent permission checks.
    SetPermissionMode {
        mode: agent_tools::PermissionMode,
    },
}

/// Read-only shared state passed to components on every event.
#[allow(dead_code)]
pub struct EventContext<'a> {
    pub focus: FocusTarget,
    pub current_session: &'a agent_core::projection::SessionProjection,
    pub projects: &'a [ProjectInfo],
    pub sessions: &'a [SessionInfo],
    pub model_profile: &'a str,
    pub permission_mode: agent_tools::PermissionMode,
    pub sidebar_left_visible: bool,
    pub sidebar_right_visible: bool,
    pub workspace_id: &'a agent_core::WorkspaceId,
    pub current_session_id: &'a Option<agent_core::SessionId>,
}

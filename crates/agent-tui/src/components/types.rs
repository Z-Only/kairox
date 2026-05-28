use agent_core::{
    AttachmentInfo, ProjectGitStatus, ProjectId, ProjectInstructionSummary,
    ProjectSessionVisibility, SessionId,
};

use super::FocusTarget;

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
    /// Approval axis label (e.g. `on_request`).
    pub approval_policy: String,
    /// Sandbox axis label (e.g. `workspace_write`).
    pub sandbox_policy: String,
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

pub mod chat;
pub mod command_palette;
pub mod mcp_overlay;
pub mod permission_modal;
pub mod sessions;
pub mod skills_overlay;
pub mod status_bar;
pub mod trace;

use agent_core::SessionId;
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
pub struct SessionInfo {
    pub id: SessionId,
    pub title: String,
    pub model_profile: String,
    pub state: SessionState,
    pub pinned: bool,
}

/// A message typed while the session is busy and held until the session
/// returns to idle. Mirrors the GUI `QueuedMessage` contract (attachments are
/// not yet supported in the TUI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedMessage {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusInfo {
    pub profile: String,
    pub permission_mode: String,
    pub session_count: usize,
    pub mcp_server_count: usize,
    pub hint: String,
    pub error: Option<String>,
    /// P3: latest `ContextAssembled.usage`. `None` until the first event.
    pub context_usage: Option<agent_core::context_types::ContextUsage>,
    /// P3: `true` between `ContextCompactionStarted` and `Completed`/`Failed`.
    pub compacting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CrossPanelEffect {
    SwitchFocus(FocusTarget),
    ShowPermissionPrompt(PermissionRequest),
    DismissPermissionPrompt,
    UpdateSessionList(Vec<SessionInfo>),
    SetStatus(StatusInfo),
    NavigateToSession(SessionId),
    StartStreaming,
    StopStreaming,
    ShowMcpOverlay(Vec<McpServerEntry>),
    DismissMcpOverlay,
    /// Open the command palette overlay.
    ShowCommandPalette,
    /// Close the command palette overlay.
    DismissCommandPalette,
    /// Insert the given text at the start of the chat input and place the
    /// cursor at the end. Used by the command palette to hand back a slash
    /// prefix that needs an argument.
    PrefillChatInput(String),
    /// Build/refresh the skills overlay snapshot and open it.
    ShowSkillsOverlay(Vec<SkillEntry>),
    /// Close the skills overlay.
    DismissSkillsOverlay,
    /// Deliver a skill's rendered markdown body to the open overlay so it
    /// can switch to inline detail view.
    ShowSkillBody {
        skill_id: String,
        body: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Command {
    SendMessage {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
        content: String,
    },
    DecidePermission {
        request_id: String,
        approved: bool,
    },
    /// Trust an MCP server so future tool calls from it are auto-approved.
    TrustMcpServer {
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
    CancelSession {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
    },
    StartSession {
        workspace_id: agent_core::WorkspaceId,
        model_profile: String,
    },
    SwitchSession {
        session_id: SessionId,
    },
    /// P3: user typed `:compact` in the chat panel; ask the runtime to
    /// summarise older history into a compaction summary.
    CompactSession {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
    },
    /// P4: user typed `:model <alias>` in the chat panel; ask the runtime
    /// to switch the active model profile mid-session. `workspace_id` is
    /// carried for symmetry with sibling variants. The TUI command leaves
    /// reasoning effort unset and uses the runtime default for reasoning
    /// models.
    SwitchModel {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
        alias: String,
    },
    /// Build a skill snapshot and open the skills overlay (Ctrl+S or
    /// `:skills` typed in the chat panel).
    OpenSkillsOverlay,
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
    pub sessions: &'a [SessionInfo],
    pub model_profile: &'a str,
    pub permission_mode: agent_tools::PermissionMode,
    pub sidebar_left_visible: bool,
    pub sidebar_right_visible: bool,
    pub workspace_id: &'a agent_core::WorkspaceId,
    pub current_session_id: &'a Option<agent_core::SessionId>,
}

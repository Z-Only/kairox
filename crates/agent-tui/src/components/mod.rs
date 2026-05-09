pub mod chat;
pub mod permission_modal;
pub mod sessions;
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
    /// carried for symmetry with sibling variants even though
    /// `LocalRuntime::switch_model` only needs `(SessionId, alias)`.
    SwitchModel {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
        alias: String,
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

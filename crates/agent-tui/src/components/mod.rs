pub mod chat;
pub mod status_bar;

use agent_core::SessionId;
use ratatui::layout::Rect;
use ratatui::Frame;

/// A self-contained UI panel that handles events and renders itself.
///
/// Components never directly reference other components.
/// Cross-panel communication flows exclusively through `CrossPanelEffect`
/// routed by the App layer.
pub trait Component {
    /// Process an incoming event. Returns (cross-panel effects, runtime commands).
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    pub request_id: String,
    pub tool_id: String,
    pub tool_preview: String,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub hint: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    CancelSession {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
    },
    StartSession {
        workspace_id: agent_core::WorkspaceId,
        model_profile: String,
    },
}

/// Read-only shared state passed to components on every event.
pub struct EventContext<'a> {
    pub focus: FocusTarget,
    pub current_session: &'a agent_core::projection::SessionProjection,
    pub sessions: &'a [SessionInfo],
    pub model_profile: &'a str,
    pub permission_mode: agent_tools::PermissionMode,
    pub sidebar_left_visible: bool,
    pub sidebar_right_visible: bool,
}

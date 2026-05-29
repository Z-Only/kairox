pub mod agent_overlay;
pub mod chat;
pub mod command_palette;
mod command_types;
mod commands;
mod effects;
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
mod types;

pub use commands::*;
pub use effects::*;
pub use types::*;

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

/// Read-only shared state passed to components on every event.
#[allow(dead_code)]
pub struct EventContext<'a> {
    pub focus: FocusTarget,
    pub current_session: &'a agent_core::projection::SessionProjection,
    pub projects: &'a [ProjectInfo],
    pub sessions: &'a [SessionInfo],
    pub model_profile: &'a str,
    pub sidebar_left_visible: bool,
    pub sidebar_right_visible: bool,
    pub workspace_id: &'a agent_core::WorkspaceId,
    pub current_session_id: &'a Option<agent_core::SessionId>,
}

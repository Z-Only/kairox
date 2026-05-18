//! ChatPanel component — message display and text input for the interactive TUI.

mod input;
mod render;

pub use render::render_messages;

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app_state::{InputMode, InputState};
use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, FocusTarget, RiskLevel,
};

// ---------------------------------------------------------------------------
// ChatPanel
// ---------------------------------------------------------------------------

/// The main chat panel: displays messages from a [`SessionProjection`] and
/// handles text input, history navigation, and permission decisions.
#[allow(dead_code)]
pub struct ChatPanel {
    focused: bool,
    pub input_content: String,
    pub input_cursor: usize,
    pub input_mode: InputMode,
    pub input_state: InputState,
    pub input_history: Vec<String>,
    /// `None` means we're at the "live" position (not browsing history).
    pub input_history_index: Option<usize>,
    pub scroll_offset: usize,
}

impl ChatPanel {
    pub fn new() -> Self {
        Self {
            focused: false,
            input_content: String::new(),
            input_cursor: 0,
            input_mode: InputMode::SingleLine,
            input_state: InputState::Normal,
            input_history: Vec::new(),
            input_history_index: None,
            scroll_offset: 0,
        }
    }
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ChatPanel {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let permission_pending = matches!(self.input_state, InputState::PermissionWait { .. });
        let action = crate::keybindings::resolve_key(
            match event {
                crossterm::event::Event::Key(ke) => *ke,
                _ => return (Vec::new(), Vec::new()),
            },
            FocusTarget::Chat,
            permission_pending,
            self.input_mode,
        );
        self.apply_key_action(action, ctx)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowPermissionPrompt(req) if req.risk_level == RiskLevel::Write => {
                // Only handle Write-level risks in ChatPanel.
                // Destructive risks are handled by PermissionModal.
                self.input_state = InputState::PermissionWait {
                    request_id: req.request_id.clone(),
                    pending_prompt: req.tool_preview.clone(),
                };
            }
            CrossPanelEffect::ShowPermissionPrompt(_) => {}
            CrossPanelEffect::DismissPermissionPrompt => {
                if matches!(self.input_state, InputState::PermissionWait { .. }) {
                    self.input_state = InputState::Normal;
                }
            }
            CrossPanelEffect::StartStreaming | CrossPanelEffect::StopStreaming => {
                // No-op for now; will be wired to RenderScheduler later.
            }
            _ => {}
        }
    }

    /// Placeholder — the App layer handles ChatPanel rendering centrally
    /// via [`render_messages`].
    fn render(&self, _area: Rect, _frame: &mut Frame) {}

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;

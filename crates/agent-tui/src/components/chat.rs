//! ChatPanel component — message display and text input for the interactive TUI.

mod input;
mod render;

pub use render::{render_messages, render_queue_strip};

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app_state::{InputMode, InputState};
use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, FocusTarget, QueuedMessage, RiskLevel,
    SessionState,
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
    /// Messages typed while the session was busy. Drained in FIFO order when
    /// the session returns to idle (see `drain_queue`).
    pub message_queue: Vec<QueuedMessage>,
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
            message_queue: Vec::new(),
        }
    }

    /// Drain all queued messages in FIFO order.
    pub fn drain_queue(&mut self) -> Vec<QueuedMessage> {
        std::mem::take(&mut self.message_queue)
    }
}

/// Whether the current session is busy (running) — Enter typed in this state
/// must enqueue rather than send. Matches the GUI `isQueueing` semantics:
/// busy when the assistant is streaming tokens, a tool is running, or the
/// session is awaiting permission.
pub(crate) fn is_session_busy(ctx: &EventContext) -> bool {
    if !ctx.current_session.token_stream.is_empty() {
        return true;
    }
    let Some(sid) = ctx.current_session_id else {
        return false;
    };
    ctx.sessions
        .iter()
        .find(|s| s.id == *sid)
        .map(|s| {
            matches!(
                s.state,
                SessionState::Active | SessionState::AwaitingPermission
            )
        })
        .unwrap_or(false)
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
            CrossPanelEffect::PrefillChatInput(text) => {
                self.input_content = text.clone();
                self.input_cursor = text.len();
                self.input_history_index = None;
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

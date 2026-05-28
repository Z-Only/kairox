mod keys;
mod render;
mod state;
mod types;

#[cfg(test)]
mod tests;

use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

pub use state::PermissionModal;

impl Component for PermissionModal {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };

        self.handle_key_event(key.code)
            .unwrap_or_else(|| (Vec::new(), Vec::new()))
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowPermissionPrompt(req) => {
                self.enqueue_request(req.clone());
            }
            CrossPanelEffect::ResolvePermissionPrompt {
                request_id,
                approved,
            } => {
                self.resolve_request(request_id, *approved);
            }
            CrossPanelEffect::DismissPermissionPrompt => {
                // Chat also consumes this effect to leave PermissionWait.
                // Queue removal is handled by explicit decisions or resolved events.
            }
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if self.request.is_some() {
            render::render_permission_modal(area, frame, self);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

//! Help overlay for global and context-specific TUI shortcuts.
//!
//! State and behaviour live in [`state`], rendering helpers live in
//! [`render`]. The [`Component`] implementation lives here so it stays
//! close to the public surface that other components use through
//! `crate::components::help_overlay::HelpOverlay`.

mod render;
mod state;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

pub use render::render_help_overlay;
pub use state::HelpOverlay;

impl Component for HelpOverlay {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        if !self.visible {
            return (Vec::new(), Vec::new());
        }

        match key.code {
            KeyCode::Esc | KeyCode::F(1) => {
                self.hide();
                (vec![CrossPanelEffect::DismissHelpOverlay], Vec::new())
            }
            _ => (Vec::new(), Vec::new()),
        }
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowHelpOverlay(snapshot) => self.show(*snapshot),
            CrossPanelEffect::DismissHelpOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if self.visible {
            render_help_overlay(area, frame, self);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

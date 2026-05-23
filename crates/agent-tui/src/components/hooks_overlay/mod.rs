//! Hooks settings overlay for managing user/project command hooks from the TUI.
//!
//! State and behaviour live in [`state`], rendering helpers live in
//! [`render`], and tests live in [`tests`]. The [`Component`] implementation
//! lives here so it stays close to the public surface that other components
//! use through `crate::components::hooks_overlay::HooksOverlay`.

mod render;
mod state;

#[cfg(test)]
mod tests;

use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

pub use state::HooksOverlay;

impl Component for HooksOverlay {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(event)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowHooksOverlay(view) => self.show(view.clone()),
            CrossPanelEffect::DismissHooksOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if self.visible {
            render::render_hooks_overlay(area, frame, self);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

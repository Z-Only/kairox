//! Agent settings manager overlay — TUI access to the same custom agent
//! profiles managed by the GUI settings pane.
//!
//! State and behaviour live in [`state`], key-event handlers live in
//! [`keys`], rendering helpers live in [`render`], and tests live in
//! [`tests`]. The [`Component`] implementation lives here so it stays
//! close to the public surface that other components use through
//! `crate::components::agent_overlay::AgentOverlay`.

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

#[allow(unused_imports)]
pub use render::render_agent_overlay;
pub use state::AgentOverlay;

impl Component for AgentOverlay {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(event)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowAgentSettingsOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissAgentSettingsOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if self.visible {
            render::render_agent_overlay(area, frame, self);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

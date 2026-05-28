//! Model profile manager overlay — pop-up modal listing profile settings with
//! the current profile/effort highlighted. It keeps the fast model switch path
//! while exposing the same first-pass settings actions as the GUI model pane.
//!
//! State and behaviour live in [`state`], key-event handlers live in
//! [`keys`], rendering helpers live in [`render`], and tests live in
//! [`tests`]. The [`Component`] implementation lives here so it stays
//! close to the public surface that other components use through
//! `crate::components::model_overlay::ModelOverlay`.

mod keys;
mod render;
mod state;

#[cfg(test)]
mod tests;

use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

// These re-exports preserve the pre-split public API for symbols that are not
// referenced elsewhere in the workspace today.
#[allow(unused_imports)]
pub use render::render_model_overlay;
pub use state::ModelOverlay;
#[allow(unused_imports)]
pub use state::REASONING_EFFORTS;

impl Component for ModelOverlay {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(ctx, event)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowModelOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::ModelProfileTested(result) => self.set_test_result(result.clone()),
            CrossPanelEffect::DismissModelOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let mut list_state = self.list_state;
        let mut effort_state = self.effort_state;
        render::render_model_overlay(area, frame, self, &mut list_state, &mut effort_state);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

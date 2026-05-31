//! Monitor overlay — lists active background monitors and allows stopping them.
//!
//! The App builds a [`MonitorOverlaySnapshot`] from the runtime's
//! `MonitorRegistry`; the overlay only owns selection state and emits
//! stop/refresh commands.

mod keys;
mod render;
mod state;
pub(crate) mod types;

#[cfg(test)]
mod tests;

use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

pub use render::render_monitor_overlay;
pub use state::MonitorOverlay;

impl Component for MonitorOverlay {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(event)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowMonitorOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissMonitorOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let mut list_state = self.list_state;
        render_monitor_overlay(area, frame, self, &mut list_state);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

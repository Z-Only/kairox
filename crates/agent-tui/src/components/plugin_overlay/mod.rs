//! Plugin manager overlay — compact keyboard surface over installed plugins,
//! catalog entries, and marketplace sources.
//!
//! The App builds a [`PluginOverlaySnapshot`] from the existing plugin facade;
//! the overlay only owns selection state and emits mutation commands.
//!
//! State and behaviour live in [`state`], key-event handlers live in
//! [`keys`], rendering helpers live in [`render`], and tests live in
//! [`tests`]. The [`Component`] implementation lives here so it stays
//! close to the public surface that other components use through
//! `crate::components::plugin_overlay::PluginOverlay`.

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

pub use render::render_plugin_overlay;
pub use state::PluginOverlay;

impl Component for PluginOverlay {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(event)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowPluginsOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissPluginsOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let mut plugins_state = self.plugins_state;
        let mut catalog_state = self.catalog_state;
        let mut sources_state = self.sources_state;
        render_plugin_overlay(
            area,
            frame,
            self,
            &mut plugins_state,
            &mut catalog_state,
            &mut sources_state,
        );
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

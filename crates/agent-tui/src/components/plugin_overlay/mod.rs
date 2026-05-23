//! Plugin manager overlay — compact keyboard surface over installed plugins,
//! catalog entries, and marketplace sources.
//!
//! The App builds a [`PluginOverlaySnapshot`] from the existing plugin facade;
//! the overlay only owns selection state and emits mutation commands.

mod render;
mod state;

#[cfg(test)]
mod tests;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

pub use render::render_plugin_overlay;
pub use state::PluginOverlay;
use state::PluginOverlayMode;

impl Component for PluginOverlay {
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

        let mut effects = Vec::new();
        let mut commands = Vec::new();

        if self.mode == PluginOverlayMode::CatalogSearch {
            if self.handle_catalog_search_key(key.code, key.modifiers) {
                commands.push(Command::OpenPluginsOverlay);
            }
            return (effects, commands);
        }

        match key.code {
            KeyCode::Tab => {
                self.tab = self.tab.next();
                self.ensure_selection();
            }
            KeyCode::BackTab => {
                self.tab = self.tab.previous();
                self.ensure_selection();
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('/') if self.tab == state::PluginTab::Catalog => {
                self.mode = PluginOverlayMode::CatalogSearch;
            }
            KeyCode::Char('s') | KeyCode::Char('S') if self.tab == state::PluginTab::Catalog => {
                self.cycle_catalog_marketplace_filter();
                commands.push(Command::OpenPluginsOverlay);
            }
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissPluginsOverlay);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => commands.push(Command::OpenPluginsOverlay),
            key => {
                if let Some(command) = self.command_for_current_tab(key) {
                    commands.push(command);
                }
            }
        }

        (effects, commands)
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

//! MCP server overlay — pop-up modal listing runtime servers, settings,
//! installed marketplace entries, catalog entries, and catalog sources.
//!
//! The App constructs a snapshot before opening the overlay; the overlay owns
//! tab and selection state, then emits [`Command`] values that the main loop
//! dispatches to the runtime manager or MCP facade.

mod editor;
mod keys;
mod render;
mod state;
mod types;

#[cfg(test)]
mod tests;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

pub use state::McpOverlay;
use types::{resource_preview_key, McpHealthState, McpOverlayMode, McpOverlayTab};

impl Component for McpOverlay {
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

        match self.mode {
            McpOverlayMode::ServerEditor => {
                commands.extend(self.handle_server_editor_key(key.code, key.modifiers));
                return (effects, commands);
            }
            McpOverlayMode::SourceEditor => {
                commands.extend(self.handle_source_editor_key(key.code, key.modifiers));
                return (effects, commands);
            }
            McpOverlayMode::CatalogFilter => {
                self.handle_catalog_filter_key(key.code, key.modifiers);
                return (effects, commands);
            }
            McpOverlayMode::CatalogInstallConfig => {
                commands.extend(self.handle_catalog_install_config_key(key.code, key.modifiers));
                return (effects, commands);
            }
            McpOverlayMode::List => {}
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
            KeyCode::Char('n') | KeyCode::Char('N') if self.tab == McpOverlayTab::Settings => {
                self.start_server_create();
            }
            KeyCode::Enter if self.tab == McpOverlayTab::Settings => {
                self.start_server_edit_selected();
            }
            KeyCode::Char('n') | KeyCode::Char('N') if self.tab == McpOverlayTab::Sources => {
                self.start_source_create();
            }
            KeyCode::Char('/') if self.tab == McpOverlayTab::Catalog => {
                self.mode = McpOverlayMode::CatalogFilter;
            }
            KeyCode::Char('t') | KeyCode::Char('T') if self.tab == McpOverlayTab::Catalog => {
                self.cycle_catalog_trust_filter();
            }
            KeyCode::Char('i') | KeyCode::Char('I') if self.tab == McpOverlayTab::Catalog => {
                commands.extend(self.start_catalog_install_selected());
            }
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissMcpOverlay);
            }
            KeyCode::Char('r') | KeyCode::Char('R')
                if matches!(
                    self.tab,
                    McpOverlayTab::Settings
                        | McpOverlayTab::Installed
                        | McpOverlayTab::Catalog
                        | McpOverlayTab::Sources
                ) =>
            {
                commands.push(Command::OpenMcpOverlay);
            }
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
            CrossPanelEffect::ShowMcpOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissMcpOverlay => self.hide(),
            CrossPanelEffect::McpToolsLoaded {
                server_id,
                tools,
                healthy,
                error,
            } => {
                self.tools.insert(server_id.clone(), tools.clone());
                self.health.insert(
                    server_id.clone(),
                    McpHealthState {
                        healthy: *healthy,
                        tool_count: tools.len(),
                        error: error.clone(),
                    },
                );
                self.ensure_selection();
            }
            CrossPanelEffect::McpConnectivityChecked(entry) => {
                self.connectivity
                    .insert(entry.server_id.clone(), entry.clone());
            }
            CrossPanelEffect::McpResourcesLoaded {
                server_id,
                resources,
            } => {
                self.resources.insert(server_id.clone(), resources.clone());
                self.ensure_selection();
            }
            CrossPanelEffect::McpPromptsLoaded { server_id, prompts } => {
                self.prompts.insert(server_id.clone(), prompts.clone());
                self.ensure_selection();
            }
            CrossPanelEffect::McpResourceRead {
                server_id,
                uri,
                preview,
            } => {
                self.resource_previews
                    .insert(resource_preview_key(server_id, uri), preview.clone());
            }
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        render::render_mcp_overlay(area, frame, self);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

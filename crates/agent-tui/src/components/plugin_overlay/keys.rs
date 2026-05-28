//! Key-event handlers for [`PluginOverlay`].
//!
//! Separated from [`super::state`] to keep the data model and selection queries
//! in one file and the interactive key-handling logic in another.

use agent_core::facade::{InstallPluginRequest, PluginInstallTarget};
use agent_core::ConfigScope;
use crossterm::event::{Event, KeyCode, KeyModifiers};

use super::state::PluginOverlay;
use super::types::{PluginOverlayMode, PluginTab};
use crate::components::{Command, CrossPanelEffect};

impl PluginOverlay {
    pub(super) fn move_down(&mut self) {
        let len = self.current_len();
        if len == 0 {
            return;
        }
        let next = match self.current_selected() {
            Some(i) if i + 1 < len => i + 1,
            Some(_) => len - 1,
            None => 0,
        };
        self.select_current(Some(next));
    }

    pub(super) fn move_up(&mut self) {
        if self.current_len() == 0 {
            return;
        }
        let next = match self.current_selected() {
            Some(i) if i > 0 => i - 1,
            _ => 0,
        };
        self.select_current(Some(next));
    }

    pub(super) fn handle_catalog_search_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> bool {
        match code {
            KeyCode::Enter | KeyCode::Esc => {
                self.mode = PluginOverlayMode::List;
                true
            }
            KeyCode::Backspace => {
                self.catalog_keyword.pop();
                false
            }
            KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.catalog_keyword.clear();
                false
            }
            KeyCode::Char(ch)
                if !modifiers.contains(KeyModifiers::CONTROL)
                    && !modifiers.contains(KeyModifiers::ALT) =>
            {
                self.catalog_keyword.push(ch);
                false
            }
            _ => false,
        }
    }

    fn toggle_install_target(&mut self) {
        self.install_target = match self.install_target {
            PluginInstallTarget::User => PluginInstallTarget::Project,
            PluginInstallTarget::Project => PluginInstallTarget::User,
        };
    }

    pub(super) fn command_for_current_tab(&mut self, key: KeyCode) -> Option<Command> {
        match (self.tab, key) {
            (PluginTab::Installed, KeyCode::Char('e') | KeyCode::Char('E')) => self
                .selected_plugin()
                .filter(|plugin| plugin.scope != ConfigScope::Builtin)
                .map(|plugin| Command::SetPluginEnabled {
                    settings_id: plugin.settings_id.clone(),
                    enabled: !plugin.enabled,
                }),
            (PluginTab::Installed, KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete) => {
                self.selected_plugin()
                    .filter(|plugin| plugin.scope != ConfigScope::Builtin)
                    .map(|plugin| Command::DeletePluginSettings {
                        settings_id: plugin.settings_id.clone(),
                    })
            }
            (PluginTab::Catalog, KeyCode::Char('i') | KeyCode::Char('I')) => self
                .selected_catalog_entry()
                .map(|entry| Command::InstallPlugin {
                    request: InstallPluginRequest {
                        marketplace_id: entry.marketplace_id.clone(),
                        plugin_name: entry.name.clone(),
                        target: self.install_target,
                    },
                }),
            (PluginTab::Catalog, KeyCode::Char('t') | KeyCode::Char('T')) => {
                self.toggle_install_target();
                None
            }
            (PluginTab::Sources, KeyCode::Char('e') | KeyCode::Char('E')) => self
                .selected_source()
                .map(|source| Command::SetPluginMarketplaceSourceEnabled {
                    source_id: source.id.clone(),
                    enabled: !source.enabled,
                }),
            _ => None,
        }
    }

    pub(super) fn handle_key_event(
        &mut self,
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
            KeyCode::Char('/') if self.tab == PluginTab::Catalog => {
                self.mode = PluginOverlayMode::CatalogSearch;
            }
            KeyCode::Char('s') | KeyCode::Char('S') if self.tab == PluginTab::Catalog => {
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
}

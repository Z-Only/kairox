//! Key-event handlers for [`SkillsOverlay`].
//!
//! Separated from [`super::state`] to keep the data model and selection queries
//! in one file and the interactive key-handling logic in another.

use crossterm::event::{Event, KeyCode};

use super::state::SkillsOverlay;
use super::types::{SkillOverlayMode, SkillTab};
use crate::components::{Command, CrossPanelEffect, EventContext};

impl SkillsOverlay {
    fn move_down(&mut self) {
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

    fn move_up(&mut self) {
        if self.current_len() == 0 {
            return;
        }
        let next = match self.current_selected() {
            Some(i) if i > 0 => i - 1,
            _ => 0,
        };
        self.select_current(Some(next));
    }

    fn toggle_install_target(&mut self) {
        self.install_target = match self.install_target {
            agent_core::facade::SkillInstallTarget::User => {
                agent_core::facade::SkillInstallTarget::Project
            }
            agent_core::facade::SkillInstallTarget::Project => {
                agent_core::facade::SkillInstallTarget::User
            }
        };
    }

    fn start_source_create(&mut self) {
        self.mode = SkillOverlayMode::SourceEditor;
        self.source_draft = super::editor::SkillSourceDraft::new();
        self.source_field_index = 0;
    }

    fn handle_catalog_detail_key(&mut self, key: KeyCode) -> Vec<Command> {
        match key {
            KeyCode::Esc => self.mode = SkillOverlayMode::List,
            KeyCode::Enter | KeyCode::Char('i') | KeyCode::Char('I') => {
                return self
                    .install_selected_catalog_command()
                    .into_iter()
                    .collect();
            }
            KeyCode::Char('t') | KeyCode::Char('T') => self.toggle_install_target(),
            _ => {}
        }
        Vec::new()
    }

    fn command_for_current_tab(&mut self, key: KeyCode) -> Option<Command> {
        match (self.tab, key) {
            (SkillTab::Installed, KeyCode::Char('e') | KeyCode::Char('E')) => self
                .selected_installed()
                .filter(|skill| {
                    skill.editable && skill.scope != agent_core::facade::SkillSettingsScope::Builtin
                })
                .map(|skill| Command::SetSkillEnabled {
                    skill_id: skill.id.clone(),
                    enabled: !skill.enabled,
                }),
            (SkillTab::Installed, KeyCode::Char('u') | KeyCode::Char('U')) => self
                .selected_installed()
                .filter(|skill| {
                    skill.install_source != agent_core::facade::SkillInstallSource::Builtin
                })
                .map(|skill| Command::UpdateSkillSettings {
                    skill_id: skill.id.clone(),
                }),
            (SkillTab::Installed, KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete) => {
                self.selected_installed()
                    .filter(|skill| skill.deletable)
                    .map(|skill| Command::DeleteSkillSettings {
                        skill_id: skill.id.clone(),
                    })
            }
            (SkillTab::Catalog, KeyCode::Char('i') | KeyCode::Char('I')) => {
                self.install_selected_catalog_command()
            }
            (SkillTab::Catalog, KeyCode::Char('t') | KeyCode::Char('T')) => {
                self.toggle_install_target();
                None
            }
            (SkillTab::Sources, KeyCode::Char('e') | KeyCode::Char('E')) => self
                .selected_source()
                .map(|source| Command::SetSkillSourceEnabled {
                    source_id: source.id.clone(),
                    enabled: !source.enabled,
                }),
            (SkillTab::Sources, KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete) => self
                .selected_source()
                .map(|source| Command::RemoveSkillSource {
                    source_id: source.id.clone(),
                }),
            (SkillTab::Search, KeyCode::Char('i') | KeyCode::Char('I')) => {
                self.install_selected_search_result_command()
            }
            (SkillTab::Search, KeyCode::Char('t') | KeyCode::Char('T')) => {
                self.toggle_install_target();
                None
            }
            _ => None,
        }
    }

    pub(super) fn handle_key_event(
        &mut self,
        ctx: &EventContext,
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

        // In body view Esc returns to the list; any other key is ignored
        // so the parent activate/deactivate keys don't fire by accident.
        if self.body.is_some() {
            if matches!(key.code, KeyCode::Esc) {
                self.body = None;
            }
            return (effects, commands);
        }

        match self.mode {
            SkillOverlayMode::SourceEditor => {
                commands.extend(self.handle_source_editor_key(key.code, key.modifiers));
                return (effects, commands);
            }
            SkillOverlayMode::CatalogDetail => {
                commands.extend(self.handle_catalog_detail_key(key.code));
                return (effects, commands);
            }
            SkillOverlayMode::CatalogFilter => {
                commands.extend(self.handle_catalog_filter_key(key.code, key.modifiers));
                return (effects, commands);
            }
            SkillOverlayMode::RemoteSearchInput => {
                commands.extend(self.handle_remote_search_input_key(key.code, key.modifiers));
                return (effects, commands);
            }
            SkillOverlayMode::List => {}
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
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissSkillsOverlay);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                commands.push(self.refresh_catalog_command());
            }
            KeyCode::Char('/') if self.tab == SkillTab::Catalog => {
                self.catalog_keyword_draft = self.catalog_keyword.clone();
                self.mode = SkillOverlayMode::CatalogFilter;
            }
            KeyCode::Char('/') if self.tab == SkillTab::Search => {
                self.search_query_draft = self.search_query.clone();
                self.mode = SkillOverlayMode::RemoteSearchInput;
            }
            KeyCode::Char('s') | KeyCode::Char('S') if self.tab == SkillTab::Catalog => {
                self.cycle_catalog_source_filter();
                commands.push(self.list_catalog_command());
            }
            KeyCode::Char('n') | KeyCode::Char('N') if self.tab == SkillTab::Sources => {
                self.start_source_create();
            }
            KeyCode::Enter => match self.tab {
                SkillTab::Discovered => {
                    if let Some(entry) = self.selected_discovered() {
                        commands.push(Command::ShowSkill {
                            skill_id: entry.id.clone(),
                        });
                    }
                }
                SkillTab::Catalog => {
                    if self.selected_catalog_entry().is_some() {
                        self.mode = SkillOverlayMode::CatalogDetail;
                    }
                }
                SkillTab::Search => {
                    if let Some(cmd) = self.install_selected_search_result_command() {
                        commands.push(cmd);
                    }
                }
                SkillTab::Installed | SkillTab::Sources => {}
            },
            KeyCode::Char('a') | KeyCode::Char('A') => {
                if let (Some(entry), Some(session_id)) =
                    (self.selected_discovered(), ctx.current_session_id.as_ref())
                {
                    if self.tab == SkillTab::Discovered && !entry.active {
                        commands.push(Command::ActivateSkill {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            skill_id: entry.id.clone(),
                        });
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let (Some(entry), Some(session_id)) =
                    (self.selected_discovered(), ctx.current_session_id.as_ref())
                {
                    if self.tab == SkillTab::Discovered && entry.active {
                        commands.push(Command::DeactivateSkill {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            skill_id: entry.id.clone(),
                        });
                    }
                }
            }
            key => {
                if let Some(command) = self.command_for_current_tab(key) {
                    commands.push(command);
                }
            }
        }

        (effects, commands)
    }
}

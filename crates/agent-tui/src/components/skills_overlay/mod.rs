//! Skills overlay — pop-up modal listing native skills with an active marker,
//! supporting per-session activation/deactivation and inline body preview.
//!
//! The TUI surface for the same data the GUI's `SkillSettingsPane` shows.
//! The App constructs a snapshot before opening the overlay; the overlay owns
//! tab and selection state, then emits [`Command`] values that the main loop
//! dispatches back to `AppFacade`.

mod editor;
mod render;
mod state;

#[cfg(test)]
mod tests;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

pub use state::{BodyView, SkillsOverlay};
use state::{SkillOverlayMode, SkillTab};

impl Component for SkillsOverlay {
    fn handle_event(
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

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowSkillsOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissSkillsOverlay => self.hide(),
            CrossPanelEffect::ShowSkillBody { skill_id, body } if self.visible => {
                self.body = Some(BodyView {
                    skill_id: skill_id.clone(),
                    body: body.clone(),
                });
            }
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        render::render_skills_overlay(area, frame, self);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

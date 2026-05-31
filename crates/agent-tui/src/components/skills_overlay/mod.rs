//! Skills overlay — pop-up modal listing native skills with an active marker,
//! supporting per-session activation/deactivation and inline body preview.
//!
//! The TUI surface for the same data the GUI's `SkillSettingsPane` shows.
//! The App constructs a snapshot before opening the overlay; the overlay owns
//! tab and selection state, then emits [`Command`] values that the main loop
//! dispatches back to `AppFacade`.

mod editor;
mod keys;
mod render;
mod render_items;
mod state;
mod types;

#[cfg(test)]
mod tests;

use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

pub use state::SkillsOverlay;
pub use types::BodyView;

impl Component for SkillsOverlay {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(ctx, event)
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
            CrossPanelEffect::SkillRemoteSearchResults(results) if self.visible => {
                self.search_results = results.clone();
                if self.search_results.is_empty() {
                    self.search_state.select(None);
                } else {
                    self.search_state.select(Some(0));
                }
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

//! Command palette — discoverable overlay for TUI actions and slash commands.
//!
//! Search-only view over a static registry. Each entry maps to either a
//! direct [`Command`] or a chat-input prefill (e.g. `:model `) so the user
//! can finish the argument inline. The palette never reparses the existing
//! `:`-prefixed slash form; selection routes the same [`Command`] the slash
//! parser would produce, or hands the prefill back to [`ChatPanel`].

mod registry;
mod render;
mod state;
mod types;

#[cfg(test)]
mod tests;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

#[allow(unused_imports)]
pub use registry::{builtin_entries, filter_entries, prefill_text, PaletteAction, PaletteEntry};
#[allow(unused_imports)]
pub use render::render_command_palette;
pub use state::CommandPalette;

impl Component for CommandPalette {
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

        match key.code {
            KeyCode::Esc => {
                self.hide();
                (vec![CrossPanelEffect::DismissCommandPalette], Vec::new())
            }
            KeyCode::Down => {
                self.move_down();
                (Vec::new(), Vec::new())
            }
            KeyCode::Up => {
                self.move_up();
                (Vec::new(), Vec::new())
            }
            KeyCode::Enter => self.activate(ctx),
            KeyCode::Backspace => {
                self.filter.pop();
                self.clamp_selection();
                (Vec::new(), Vec::new())
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.selected = 0;
                self.clamp_selection();
                (Vec::new(), Vec::new())
            }
            _ => (Vec::new(), Vec::new()),
        }
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowCommandPalette => self.show(),
            CrossPanelEffect::DismissCommandPalette => self.hide(),
            CrossPanelEffect::UpdateCommandPalette(snapshot) => {
                self.model_profiles = snapshot.model_profiles.clone();
                self.skills = snapshot.skills.clone();
                self.clamp_selection();
            }
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let entries = self.visible_entries();
        let mut state = self.list_state;
        render_command_palette(area, frame, self, &entries, &mut state);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

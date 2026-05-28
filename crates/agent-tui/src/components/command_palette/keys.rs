//! Key-event handlers for [`CommandPalette`].
//!
//! Separated from [`super::state`] to keep the data model and helpers in one
//! file and the interactive key-handling logic in another.

use crossterm::event::KeyCode;

use super::state::CommandPalette;
use crate::components::{Command, CrossPanelEffect, EventContext};

impl CommandPalette {
    /// Process a key event while the command palette is visible.
    ///
    /// Returns `None` when the palette is hidden, signalling that the caller
    /// should fall through to the default no-op response.
    pub(super) fn handle_key_event(
        &mut self,
        ctx: &EventContext,
        code: KeyCode,
    ) -> Option<(Vec<CrossPanelEffect>, Vec<Command>)> {
        if !self.visible {
            return None;
        }

        Some(match code {
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
        })
    }
}

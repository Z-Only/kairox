//! Key-event handlers for [`InstructionsOverlay`].
//!
//! Separated from [`super::state`] to keep the data model and helpers in one
//! file and the interactive key-handling logic in another.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::state::InstructionsOverlay;
use crate::components::{Command, CrossPanelEffect};

impl InstructionsOverlay {
    /// Process a key event while the overlay is visible.
    ///
    /// Returns the pair of cross-panel effects and commands that the
    /// [`Component`] `handle_event` implementation should return.
    pub(super) fn handle_key_event(
        &mut self,
        key: &KeyEvent,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        match key.code {
            KeyCode::Esc => {
                self.hide();
                (
                    vec![CrossPanelEffect::DismissInstructionsOverlay],
                    Vec::new(),
                )
            }
            KeyCode::Tab => {
                self.tab = self.tab.next();
                (Vec::new(), Vec::new())
            }
            KeyCode::BackTab => {
                self.tab = self.tab.previous();
                (Vec::new(), Vec::new())
            }
            KeyCode::F(2) => {
                let commands = self.save_command().into_iter().collect();
                (Vec::new(), commands)
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let commands = self.save_command().into_iter().collect();
                (Vec::new(), commands)
            }
            KeyCode::Enter => {
                self.insert_newline();
                (Vec::new(), Vec::new())
            }
            KeyCode::Backspace => {
                self.backspace();
                (Vec::new(), Vec::new())
            }
            KeyCode::Char(ch) => {
                self.insert_char(ch);
                (Vec::new(), Vec::new())
            }
            _ => (Vec::new(), Vec::new()),
        }
    }
}

//! Instructions settings overlay for viewing and editing user/project
//! instructions from the TUI.
//!
//! State and behaviour live in [`state`], rendering helpers live in
//! [`render`]. The [`Component`] implementation lives here so it stays
//! close to the public surface that other components use through
//! `crate::components::instructions_overlay::InstructionsOverlay`.

mod render;
mod state;
mod types;

use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

pub use render::render_instructions_overlay;
pub use state::InstructionsOverlay;

impl Component for InstructionsOverlay {
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

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowInstructionsOverlay(view) => self.show(view.clone()),
            CrossPanelEffect::ShowSystemPromptOverlay(view) => {
                self.show_system_prompt(view.clone())
            }
            CrossPanelEffect::DismissInstructionsOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if self.visible {
            render_instructions_overlay(area, frame, self);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

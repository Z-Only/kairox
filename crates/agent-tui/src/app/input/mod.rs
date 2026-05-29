//! Input dispatch for the TUI [`App`]. The top-level
//! [`App::handle_crossterm_event`] entry point lives here. It first runs the
//! event through [`App::handle_crossterm_event_unconfirmed`] (overlay/palette
//! routing in [`palette`]), then funnels any returned [`Command`]s through
//! [`App::confirm_destructive_commands`] for two-step destructive
//! confirmation. The keymap → [`Command`] mapping lives in [`keymap`] and
//! the chat composer / queue handling lives in [`session`].

use crossterm::event::Event;

use crate::components::Command;

use super::App;

mod keymap;
mod palette;
mod session;

impl App {
    /// Handle a raw crossterm event, returning any commands to dispatch.
    pub fn handle_crossterm_event(&mut self, event: &Event) -> Vec<Command> {
        let commands = self.handle_crossterm_event_unconfirmed(event);
        self.confirm_destructive_commands(commands)
    }

    fn confirm_destructive_commands(&mut self, commands: Vec<Command>) -> Vec<Command> {
        let mut saw_destructive_command = false;
        let mut confirmed = Vec::with_capacity(commands.len());

        for command in commands {
            let Some(target) = command.destructive_confirmation_target() else {
                confirmed.push(command);
                continue;
            };

            saw_destructive_command = true;
            if self.destructive_confirmation.arm_or_confirm(target) {
                self.finalize_confirmed_destructive_command(&command);
                confirmed.push(command);
            } else if let Some(hint) = self.destructive_confirmation.pending_hint() {
                self.state.push_status_message(hint.clone());
                self.status_bar.push_notification(hint);
                self.state.render_scheduler.mark_dirty();
            }
        }

        if !saw_destructive_command {
            self.destructive_confirmation.clear();
        }

        confirmed
    }

    fn finalize_confirmed_destructive_command(&mut self, command: &Command) {
        match command {
            Command::ArchiveSession { .. } | Command::RemoveProject { .. } => {
                self.sessions.close_action_menu();
            }
            Command::DeleteSession { .. } => {
                self.sessions.close_action_menu();
                self.sessions.close_archive_manager();
            }
            _ => {}
        }
    }

    pub(super) fn current_draft_save_command(&self) -> Option<Command> {
        Some(Command::SaveDraft {
            session_id: self.current_session_id.clone()?,
            draft_text: self.chat.input_content.clone(),
        })
    }

    pub(super) fn help_overlay_snapshot(&self) -> crate::components::HelpOverlaySnapshot {
        crate::components::HelpOverlaySnapshot {
            focus: self.state.focus_manager.current(),
        }
    }
}

#[cfg(test)]
mod tests;

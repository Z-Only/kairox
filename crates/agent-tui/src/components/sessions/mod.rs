//! Sessions panel — projects + sessions list with archive manager and
//! contextual action overlay. The TUI counterpart to the GUI's session
//! sidebar.
//!
//! Split into:
//! - [`state`]: data model, selection queries, and panel methods.
//! - [`keys`]: key-event dispatch (KeyCode → actions/commands).
//! - [`actions`]: business-logic mutations invoked by key handlers.
//! - [`render`]: list layout, archive manager modal, and action overlay.
//!
//! Mirrors the layout used by `mcp_overlay` and `skills_overlay`.

mod actions;
mod keys;
mod render;
mod state;

#[cfg(test)]
mod tests;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

pub use render::render_sessions;
pub use state::{session_list_rows, SessionListRow, SessionsPanel};
// Re-exported to preserve the original `pub` surface of the pre-split module
// even though the rest of the workspace does not currently use them.
use state::SessionActionMode;
#[allow(unused_imports)]
pub use state::{ArchiveStats, SessionAction};

impl Component for SessionsPanel {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };

        if self.archive_manager_open {
            let commands = self.handle_archive_manager_key(ctx, key.code);
            return (Vec::new(), commands);
        }

        if !self.context_menu_open {
            if matches!(key.code, KeyCode::Char('x')) {
                self.open_action_menu(ctx.projects, ctx.sessions);
            }
            return (Vec::new(), Vec::new());
        }

        let commands = match self.action_mode {
            SessionActionMode::Menu => self.handle_menu_key(ctx, key.code),
            SessionActionMode::RenameSession { .. } | SessionActionMode::RenameProject { .. } => {
                self.handle_rename_key(key.code)
            }
            SessionActionMode::Worktree { .. } => self.handle_worktree_key(key.code),
        };
        (Vec::new(), commands)
    }

    fn handle_effect(&mut self, _effect: &CrossPanelEffect) {}

    fn render(&self, area: Rect, frame: &mut Frame) {
        let _ = (area, frame);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

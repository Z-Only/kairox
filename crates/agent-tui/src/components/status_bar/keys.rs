//! Key-event handlers for [`StatusBar`].
//!
//! Separated from [`super::state`] to keep the data model and helpers in one
//! file and the interactive key-handling logic in another.

use crossterm::event::{KeyCode, KeyEvent};

use super::state::StatusBar;
use crate::components::{Command, EventContext};

impl StatusBar {
    /// Process a key event while the context-details overlay is visible.
    ///
    /// Returns `None` when the overlay is hidden or the event is not a key
    /// press, signalling that the caller should fall through to the default
    /// no-op response.
    pub(super) fn handle_key_event(
        &mut self,
        ctx: &EventContext,
        key: &KeyEvent,
    ) -> Option<Vec<Command>> {
        match key.code {
            KeyCode::Esc => {
                self.close_context_details();
                Some(Vec::new())
            }
            KeyCode::Char('c') | KeyCode::Char('C')
                if self.info.context_usage.is_some() && !self.info.compacting =>
            {
                self.close_context_details();
                let session_id = ctx.current_session_id.as_ref()?;
                Some(vec![Command::CompactSession {
                    workspace_id: ctx.workspace_id.clone(),
                    session_id: session_id.clone(),
                }])
            }
            _ => Some(Vec::new()),
        }
    }
}

//! `StatusBar` component struct, constructors, and `Component` impl.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext, StatusInfo};

use super::context_overlay::render_context_details_overlay;
use super::render::render_status_bar_with_notification;

// ---------------------------------------------------------------------------
// StatusBar component
// ---------------------------------------------------------------------------

/// Read-only status bar that displays profile, permission mode, session count,
/// MCP server count, a hint, and optional error text.
pub struct StatusBar {
    pub(super) focused: bool,
    pub(super) info: StatusInfo,
    context_details_visible: bool,
    pub(super) notifications: Vec<String>,
}

impl StatusBar {
    pub(super) const NOTIFICATION_LOG_LIMIT: usize = 100;

    pub fn new() -> Self {
        Self {
            focused: false,
            info: StatusInfo {
                profile: String::new(),
                permission_mode: String::new(),
                session_count: 0,
                mcp_server_count: 0,
                session_metadata: Vec::new(),
                hint: String::new(),
                error: None,
                context_usage: None,
                compacting: false,
            },
            context_details_visible: false,
            notifications: Vec::new(),
        }
    }

    pub fn close_context_details(&mut self) {
        self.context_details_visible = false;
    }

    pub fn toggle_context_details(&mut self) {
        self.context_details_visible = !self.context_details_visible;
    }

    pub fn context_details_visible(&self) -> bool {
        self.context_details_visible
    }

    pub fn push_notification(&mut self, message: impl Into<String>) {
        let message = message.into();
        if message.trim().is_empty() {
            return;
        }
        self.notifications.push(message);
        if self.notifications.len() > Self::NOTIFICATION_LOG_LIMIT {
            let overflow = self.notifications.len() - Self::NOTIFICATION_LOG_LIMIT;
            self.notifications.drain(0..overflow);
        }
    }

    pub fn latest_notification(&self) -> Option<&str> {
        self.notifications.last().map(String::as_str)
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for StatusBar {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        if !self.context_details_visible {
            return (Vec::new(), Vec::new());
        }

        let crossterm::event::Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };

        match key.code {
            crossterm::event::KeyCode::Esc => {
                self.close_context_details();
                (Vec::new(), Vec::new())
            }
            crossterm::event::KeyCode::Char('c') | crossterm::event::KeyCode::Char('C')
                if self.info.context_usage.is_some() && !self.info.compacting =>
            {
                self.close_context_details();
                let Some(session_id) = ctx.current_session_id.as_ref() else {
                    return (Vec::new(), Vec::new());
                };
                (
                    Vec::new(),
                    vec![Command::CompactSession {
                        workspace_id: ctx.workspace_id.clone(),
                        session_id: session_id.clone(),
                    }],
                )
            }
            _ => (Vec::new(), Vec::new()),
        }
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        if let CrossPanelEffect::SetStatus(info) = effect {
            self.info = info.clone();
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        render_status_bar_with_notification(area, frame, &self.info, self.latest_notification());
        if self.context_details_visible {
            render_context_details_overlay(area, frame, &self.info);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

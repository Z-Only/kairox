mod render;
mod state;

#[cfg(test)]
mod tests;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, RiskLevel,
};

pub use state::PermissionModal;

impl Component for PermissionModal {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        let Some(req) = self.request.clone() else {
            return (Vec::new(), Vec::new());
        };

        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let Some(req) = self.resolve_active_request(true) else {
                    return (Vec::new(), Vec::new());
                };
                commands.push(Command::DecidePermission {
                    request_id: req.request_id.clone(),
                    approved: true,
                });
                effects.push(CrossPanelEffect::DismissPermissionPrompt);
            }
            KeyCode::Char('n')
            | KeyCode::Char('N')
            | KeyCode::Char('d')
            | KeyCode::Char('D')
            | KeyCode::Esc => {
                let Some(req) = self.resolve_active_request(false) else {
                    return (Vec::new(), Vec::new());
                };
                commands.push(Command::DecidePermission {
                    request_id: req.request_id.clone(),
                    approved: false,
                });
                effects.push(CrossPanelEffect::DismissPermissionPrompt);
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                // Trust the MCP server and approve this request
                if let RiskLevel::McpTool { server_id } = &req.risk_level {
                    let server_id = server_id.clone();
                    let Some(req) = self.resolve_active_request(true) else {
                        return (Vec::new(), Vec::new());
                    };
                    commands.push(Command::TrustMcpServer { server_id });
                    commands.push(Command::DecidePermission {
                        request_id: req.request_id.clone(),
                        approved: true,
                    });
                    effects.push(CrossPanelEffect::DismissPermissionPrompt);
                }
            }
            _ => {}
        }

        (effects, commands)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowPermissionPrompt(req) => {
                self.enqueue_request(req.clone());
            }
            CrossPanelEffect::ResolvePermissionPrompt {
                request_id,
                approved,
            } => {
                self.resolve_request(request_id, *approved);
            }
            CrossPanelEffect::DismissPermissionPrompt => {
                // Chat also consumes this effect to leave PermissionWait.
                // Queue removal is handled by explicit decisions or resolved events.
            }
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if self.request.is_some() {
            render::render_permission_modal(area, frame, self);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

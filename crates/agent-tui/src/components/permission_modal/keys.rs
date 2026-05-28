//! Key-event handlers for [`PermissionModal`].
//!
//! Separated from [`super::state`] to keep the data model and helpers in one
//! file and the interactive key-handling logic in another.

use crossterm::event::KeyCode;

use super::state::PermissionModal;
use crate::components::{Command, CrossPanelEffect, RiskLevel};

impl PermissionModal {
    /// Process a key event and return any resulting effects and commands.
    ///
    /// Returns `None` when there is no active permission request, signalling
    /// that the caller should fall through to the default no-op response.
    pub(super) fn handle_key_event(
        &mut self,
        code: KeyCode,
    ) -> Option<(Vec<CrossPanelEffect>, Vec<Command>)> {
        let req = self.request.clone()?;

        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let req = self.resolve_active_request(true)?;
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
                let req = self.resolve_active_request(false)?;
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
                    let req = self.resolve_active_request(true)?;
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

        Some((effects, commands))
    }
}

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, PermissionRequest, RiskLevel,
};

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PermissionHistoryEntry {
    request: PermissionRequest,
    approved: bool,
}

pub struct PermissionModal {
    focused: bool,
    pub request: Option<PermissionRequest>,
    pending_requests: Vec<PermissionRequest>,
    history: Vec<PermissionHistoryEntry>,
}

impl Default for PermissionModal {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionModal {
    pub fn new() -> Self {
        Self {
            focused: false,
            request: None,
            pending_requests: Vec::new(),
            history: Vec::new(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.request.is_some()
    }

    fn enqueue_request(&mut self, request: PermissionRequest) {
        if self
            .pending_requests
            .iter()
            .any(|pending| pending.request_id == request.request_id)
            || self
                .request
                .as_ref()
                .is_some_and(|pending| pending.request_id == request.request_id)
        {
            return;
        }

        self.pending_requests.push(request);
        if self.request.is_none() {
            self.sync_active_request();
        }
    }

    fn resolve_active_request(&mut self, approved: bool) -> Option<PermissionRequest> {
        let request_id = self.request.as_ref()?.request_id.clone();
        self.resolve_request(&request_id, approved)
    }

    fn resolve_request(&mut self, request_id: &str, approved: bool) -> Option<PermissionRequest> {
        let resolved = if let Some(index) = self
            .pending_requests
            .iter()
            .position(|pending| pending.request_id == request_id)
        {
            Some(self.pending_requests.remove(index))
        } else if self
            .request
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id)
        {
            self.request.take()
        } else {
            None
        };

        if let Some(request) = resolved.as_ref() {
            self.push_history(request.clone(), approved);
        }
        self.sync_active_request();
        resolved
    }

    fn sync_active_request(&mut self) {
        self.request = self.pending_requests.first().cloned();
    }

    fn push_history(&mut self, request: PermissionRequest, approved: bool) {
        self.history
            .push(PermissionHistoryEntry { request, approved });
        const MAX_HISTORY: usize = 6;
        if self.history.len() > MAX_HISTORY {
            let excess = self.history.len() - MAX_HISTORY;
            self.history.drain(0..excess);
        }
    }
}

pub fn render_permission_modal(area: Rect, frame: &mut Frame, modal: &PermissionModal) {
    let Some(request) = modal.request.as_ref() else {
        return;
    };

    let modal_width = 76.min(area.width.saturating_sub(4));
    let modal_height = 19.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let (title, risk_label, risk_color, warning) = match &request.risk_level {
        RiskLevel::Destructive => (
            "⛔ Destructive Operation",
            "Destructive",
            Color::Red,
            "This operation cannot be undone.",
        ),
        RiskLevel::Write => (
            "🧠 Memory Write",
            "Write",
            Color::Yellow,
            "This will save a memory entry.",
        ),
        RiskLevel::McpTool { server_id: _ } => (
            "🔌 MCP Tool",
            "MCP",
            Color::Magenta,
            "",
            // Use server_id below for the tool label
        ),
    };

    // For MCP tools, show [MCP] server/tool in the tool label
    let tool_label = match &request.risk_level {
        RiskLevel::McpTool { server_id } => {
            format!("[MCP] {}/{}", server_id, request.tool_id)
        }
        _ => request.tool_id.clone(),
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "Permission Center",
                Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {} pending", modal.pending_requests.len().max(1)),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::styled(title, Style::default().fg(risk_color))),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tool: ", Style::default().fg(Color::Gray)),
            Span::raw(&tool_label),
        ]),
        Line::from(vec![
            Span::styled("Command: ", Style::default().fg(Color::Gray)),
            Span::raw(&request.tool_preview),
        ]),
        Line::from(vec![
            Span::styled("Risk: ", Style::default().fg(Color::Gray)),
            Span::styled(risk_label, Style::default().fg(risk_color)),
        ]),
    ];
    if !warning.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(warning));
    }

    if modal.pending_requests.len() > 1 {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Pending queue",
            Style::default().fg(Color::Cyan),
        )));
        for (index, pending) in modal.pending_requests.iter().take(4).enumerate() {
            let marker = if index == 0 { ">" } else { " " };
            let label = match &pending.risk_level {
                RiskLevel::McpTool { server_id } => {
                    format!("[MCP] {server_id}/{}", pending.tool_id)
                }
                RiskLevel::Destructive => format!("[destructive] {}", pending.tool_id),
                RiskLevel::Write => format!("[write] {}", pending.tool_id),
            };
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(risk_color)),
                Span::raw(format!(" {} ", index + 1)),
                Span::raw(label),
            ]));
        }
    }

    if !modal.history.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Recent",
            Style::default().fg(Color::DarkGray),
        )));
        for entry in modal.history.iter().rev().take(2) {
            let status = if entry.approved { "allowed" } else { "denied" };
            lines.push(Line::from(vec![
                Span::styled(status, Style::default().fg(Color::DarkGray)),
                Span::raw(format!(" {}", entry.request.tool_id)),
            ]));
        }
    }
    lines.push(Line::from(""));

    // Key hints — add (T) Trust option for MCP tools
    let mut key_hints = vec![
        Span::styled("[Y] Allow once  ", Style::default().fg(Color::Yellow)),
        Span::styled("[N] Deny  ", Style::default().fg(Color::Gray)),
    ];
    if matches!(request.risk_level, RiskLevel::McpTool { .. }) {
        key_hints.push(Span::styled(
            "[T] Trust server  ",
            Style::default().fg(Color::Magenta),
        ));
    }
    key_hints.push(Span::styled(
        "[Esc] Cancel",
        Style::default().fg(Color::DarkGray),
    ));
    lines.push(Line::from(key_hints));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(risk_color)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, modal_area);
}

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
            render_permission_modal(area, frame, self);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::FocusTarget;

    fn test_ctx() -> EventContext<'static> {
        use agent_core::projection::SessionProjection;
        static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
        let projection = PROJECTION.get_or_init(SessionProjection::default);
        static SESSIONS: std::sync::OnceLock<Vec<crate::components::SessionInfo>> =
            std::sync::OnceLock::new();
        let sessions = SESSIONS.get_or_init(Vec::new);
        EventContext {
            focus: FocusTarget::Chat,
            current_session: projection,
            projects: &[],
            sessions,
            model_profile: "fake",
            sidebar_left_visible: true,
            sidebar_right_visible: true,
            workspace_id: Box::leak(Box::new(agent_core::WorkspaceId::new())),
            current_session_id: Box::leak(Box::new(None)),
        }
    }

    #[test]
    fn modal_invisible_when_no_request() {
        let modal = PermissionModal::new();
        assert!(!modal.is_visible());
    }

    #[test]
    fn modal_visible_on_destructive_effect() {
        let mut modal = PermissionModal::new();
        modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
            request_id: "req1".into(),
            tool_id: "shell.exec".into(),
            tool_preview: "rm -rf target/".into(),
            risk_level: RiskLevel::Destructive,
        }));
        assert!(modal.is_visible());
    }

    #[test]
    fn modal_visible_on_write_risk() {
        let mut modal = PermissionModal::new();
        modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
            request_id: "req2".into(),
            tool_id: "shell.exec".into(),
            tool_preview: "cargo build".into(),
            risk_level: RiskLevel::Write,
        }));
        assert!(modal.is_visible());
    }

    #[test]
    fn modal_visible_on_mcp_tool_risk() {
        let mut modal = PermissionModal::new();
        modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
            request_id: "req3".into(),
            tool_id: "echo".into(),
            tool_preview: "MCP tool invocation".into(),
            risk_level: RiskLevel::McpTool {
                server_id: "my-server".into(),
            },
        }));
        assert!(modal.is_visible());
    }

    #[test]
    fn allow_sends_decide_and_dismisses() {
        let mut modal = PermissionModal::new();
        modal.request = Some(PermissionRequest {
            request_id: "req1".into(),
            tool_id: "shell.exec".into(),
            tool_preview: "rm -rf target/".into(),
            risk_level: RiskLevel::Destructive,
        });
        let key = Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('y'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let (effects, commands) = modal.handle_event(&test_ctx(), &key);
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            Command::DecidePermission { approved: true, .. }
        ));
        assert!(effects.contains(&CrossPanelEffect::DismissPermissionPrompt));
        assert!(!modal.is_visible());
    }

    #[test]
    fn deny_sends_decide_false_and_dismisses() {
        let mut modal = PermissionModal::new();
        modal.request = Some(PermissionRequest {
            request_id: "req1".into(),
            tool_id: "shell.exec".into(),
            tool_preview: "rm -rf target/".into(),
            risk_level: RiskLevel::Destructive,
        });
        let key = Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('n'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let (effects, commands) = modal.handle_event(&test_ctx(), &key);
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            Command::DecidePermission {
                approved: false,
                ..
            }
        ));
        assert!(effects.contains(&CrossPanelEffect::DismissPermissionPrompt));
        assert!(!modal.is_visible());
    }

    #[test]
    fn escape_denies_and_dismisses() {
        let mut modal = PermissionModal::new();
        modal.request = Some(PermissionRequest {
            request_id: "req1".into(),
            tool_id: "shell.exec".into(),
            tool_preview: "rm -rf target/".into(),
            risk_level: RiskLevel::Destructive,
        });
        let key = Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        let (_, commands) = modal.handle_event(&test_ctx(), &key);
        assert!(matches!(
            &commands[0],
            Command::DecidePermission {
                approved: false,
                ..
            }
        ));
        assert!(!modal.is_visible());
    }

    #[test]
    fn trust_key_trusts_mcp_server_and_approves() {
        let mut modal = PermissionModal::new();
        modal.request = Some(PermissionRequest {
            request_id: "req1".into(),
            tool_id: "echo".into(),
            tool_preview: "MCP tool call".into(),
            risk_level: RiskLevel::McpTool {
                server_id: "my-server".into(),
            },
        });
        let key = Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('t'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let (effects, commands) = modal.handle_event(&test_ctx(), &key);
        // Should produce TrustMcpServer + DecidePermission(approved=true)
        assert_eq!(commands.len(), 2);
        assert!(matches!(
            &commands[0],
            Command::TrustMcpServer { server_id } if server_id == "my-server"
        ));
        assert!(matches!(
            &commands[1],
            Command::DecidePermission { approved: true, .. }
        ));
        assert!(effects.contains(&CrossPanelEffect::DismissPermissionPrompt));
        assert!(!modal.is_visible());
    }

    #[test]
    fn trust_key_ignored_for_non_mcp_risk() {
        let mut modal = PermissionModal::new();
        modal.request = Some(PermissionRequest {
            request_id: "req1".into(),
            tool_id: "shell.exec".into(),
            tool_preview: "rm -rf target/".into(),
            risk_level: RiskLevel::Destructive,
        });
        let key = Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('t'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let (_, commands) = modal.handle_event(&test_ctx(), &key);
        assert!(commands.is_empty());
        assert!(modal.is_visible()); // Modal stays visible
    }

    #[test]
    fn pending_prompts_queue_and_resolve_one_at_a_time() {
        let mut modal = PermissionModal::new();
        modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
            request_id: "req1".into(),
            tool_id: "shell.exec".into(),
            tool_preview: "write file".into(),
            risk_level: RiskLevel::Write,
        }));
        modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
            request_id: "req2".into(),
            tool_id: "shell.exec".into(),
            tool_preview: "delete file".into(),
            risk_level: RiskLevel::Destructive,
        }));

        assert_eq!(
            modal.request.as_ref().map(|req| req.request_id.as_str()),
            Some("req1")
        );

        let allow = Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('y'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let (_, commands) = modal.handle_event(&test_ctx(), &allow);
        assert_eq!(
            commands,
            vec![Command::DecidePermission {
                request_id: "req1".into(),
                approved: true,
            }]
        );
        assert!(modal.is_visible());
        assert_eq!(
            modal.request.as_ref().map(|req| req.request_id.as_str()),
            Some("req2")
        );

        let deny = Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('n'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let (_, commands) = modal.handle_event(&test_ctx(), &deny);
        assert_eq!(
            commands,
            vec![Command::DecidePermission {
                request_id: "req2".into(),
                approved: false,
            }]
        );
        assert!(!modal.is_visible());
    }
}

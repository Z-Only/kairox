use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, PermissionRequest, RiskLevel,
};

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

pub struct PermissionModal {
    focused: bool,
    pub request: Option<PermissionRequest>,
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
        }
    }

    pub fn is_visible(&self) -> bool {
        self.request.is_some()
    }
}

pub fn render_permission_modal(area: Rect, frame: &mut Frame, request: &PermissionRequest) {
    let modal_width = 50.min(area.width.saturating_sub(4));
    let modal_height = 10.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let (title, risk_label, risk_color, warning) = match request.risk_level {
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
    };

    let mut lines = vec![
        Line::from(Span::styled(
            title,
            Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tool: ", Style::default().fg(Color::Gray)),
            Span::raw(&request.tool_id),
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
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("[Y] Allow once  ", Style::default().fg(Color::Yellow)),
        Span::styled("[N] Deny  ", Style::default().fg(Color::Gray)),
        Span::styled("[Esc] Cancel", Style::default().fg(Color::DarkGray)),
    ]));

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
        let Some(req) = &self.request else {
            return (Vec::new(), Vec::new());
        };

        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                commands.push(Command::DecidePermission {
                    request_id: req.request_id.clone(),
                    approved: true,
                });
                effects.push(CrossPanelEffect::DismissPermissionPrompt);
                self.request = None;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                commands.push(Command::DecidePermission {
                    request_id: req.request_id.clone(),
                    approved: false,
                });
                effects.push(CrossPanelEffect::DismissPermissionPrompt);
                self.request = None;
            }
            _ => {}
        }

        (effects, commands)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowPermissionPrompt(req) => {
                // Show modal for Destructive risks and Write (memory) risks
                self.request = Some(req.clone());
            }
            CrossPanelEffect::DismissPermissionPrompt => {
                self.request = None;
            }
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if let Some(req) = &self.request {
            render_permission_modal(area, frame, req);
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
            sessions,
            model_profile: "fake",
            permission_mode: agent_tools::PermissionMode::Suggest,
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
}

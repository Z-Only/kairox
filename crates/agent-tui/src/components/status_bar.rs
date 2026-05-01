//! StatusBar component — a read-only single-line bar at the bottom of the TUI.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::{Command, Component, CrossPanelEffect, EventContext, StatusInfo};

// ---------------------------------------------------------------------------
// PermissionMode extension trait
// ---------------------------------------------------------------------------

/// Extension trait for [`agent_tools::PermissionMode`] to provide display labels.
///
/// We cannot add inherent methods to a foreign type, so we use a trait instead.
pub trait PermissionModeExt {
    /// Return a static string label for the permission mode.
    fn as_str(&self) -> &'static str;
}

impl PermissionModeExt for agent_tools::PermissionMode {
    fn as_str(&self) -> &'static str {
        match self {
            agent_tools::PermissionMode::ReadOnly => "readonly",
            agent_tools::PermissionMode::Suggest => "suggest",
            agent_tools::PermissionMode::Agent => "agent",
            agent_tools::PermissionMode::Autonomous => "autonomous",
            agent_tools::PermissionMode::Interactive => "interactive",
        }
    }
}

// ---------------------------------------------------------------------------
// StatusInfo helpers
// ---------------------------------------------------------------------------

impl StatusInfo {
    /// Return a human-readable label for the stored permission mode string.
    ///
    /// Since `permission_mode` is already a `String` set via
    /// `PermissionMode::as_str()`, we simply return it as-is.
    pub fn permission_mode_label(&self) -> &str {
        &self.permission_mode
    }
}

// ---------------------------------------------------------------------------
// StatusBar component
// ---------------------------------------------------------------------------

/// Read-only status bar that displays profile, permission mode, session count,
/// a hint, and optional error text.
pub struct StatusBar {
    focused: bool,
    info: StatusInfo,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            focused: false,
            info: StatusInfo {
                profile: String::new(),
                permission_mode: String::new(),
                session_count: 0,
                hint: String::new(),
                error: None,
            },
        }
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
        _ctx: &EventContext,
        _event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        // Status bar is display-only; it never produces effects or commands.
        (Vec::new(), Vec::new())
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        if let CrossPanelEffect::SetStatus(info) = effect {
            self.info = info.clone();
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        render_status_bar(area, frame, &self.info);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

// ---------------------------------------------------------------------------
// Standalone render helper
// ---------------------------------------------------------------------------

/// Render a single-line status bar into the given area.
///
/// Layout (left → right):
///
/// ```text
/// [ profile ] [ mode ] sessions: N  hint text  error!
/// ```
///
/// - **profile** — cyan background, bold
/// - **permission mode** — yellow background, bold
/// - **session count** — default style
/// - **hint** — dim
/// - **error** (if present) — red foreground, bold
pub fn render_status_bar(area: Rect, frame: &mut Frame, info: &StatusInfo) {
    let mut spans: Vec<Span> = Vec::new();

    // Profile badge
    spans.push(Span::styled(
        format!(" {} ", info.profile),
        Style::default()
            .bg(Color::Cyan)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::raw(" "));

    // Permission mode badge
    spans.push(Span::styled(
        format!(" {} ", info.permission_mode_label()),
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::raw(" "));

    // Session count
    spans.push(Span::styled(
        format!("sessions: {}", info.session_count),
        Style::default(),
    ));

    spans.push(Span::raw("  "));

    // Hint (dim)
    if !info.hint.is_empty() {
        spans.push(Span::styled(
            &info.hint,
            Style::default().add_modifier(Modifier::DIM),
        ));
    }

    // Error (red, bold) — prepend separator
    if let Some(err) = &info.error {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            err,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn test_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
        let backend = TestBackend::new(width, height);
        Terminal::new(backend).unwrap()
    }

    #[test]
    fn status_bar_renders_without_panic() {
        let info = StatusInfo {
            profile: "fast".to_string(),
            permission_mode: "suggest".to_string(),
            session_count: 3,
            hint: "Alt+Q quit".to_string(),
            error: None,
        };

        let mut terminal = test_terminal(80, 1);
        terminal
            .draw(|frame| {
                render_status_bar(frame.area(), frame, &info);
            })
            .expect("render should not panic");
    }

    #[test]
    fn status_bar_renders_with_error_without_panic() {
        let info = StatusInfo {
            profile: "local-code".to_string(),
            permission_mode: "agent".to_string(),
            session_count: 1,
            hint: "F1 help".to_string(),
            error: Some("connection lost".to_string()),
        };

        let mut terminal = test_terminal(80, 1);
        terminal
            .draw(|frame| {
                render_status_bar(frame.area(), frame, &info);
            })
            .expect("render should not panic");
    }

    #[test]
    fn permission_mode_as_str() {
        use PermissionModeExt;
        assert_eq!(agent_tools::PermissionMode::ReadOnly.as_str(), "readonly");
        assert_eq!(agent_tools::PermissionMode::Suggest.as_str(), "suggest");
        assert_eq!(agent_tools::PermissionMode::Agent.as_str(), "agent");
        assert_eq!(
            agent_tools::PermissionMode::Autonomous.as_str(),
            "autonomous"
        );
    }

    #[test]
    fn status_info_permission_mode_label() {
        let info = StatusInfo {
            profile: "fast".to_string(),
            permission_mode: "agent".to_string(),
            session_count: 0,
            hint: String::new(),
            error: None,
        };
        assert_eq!(info.permission_mode_label(), "agent");
    }

    #[test]
    fn status_bar_component_handle_event_returns_empty() {
        let mut bar = StatusBar::new();
        let ctx = EventContext {
            focus: super::super::FocusTarget::Chat,
            current_session: &agent_core::projection::SessionProjection::default(),
            sessions: &[],
            model_profile: "fast",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: false,
        };
        let event = crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('a'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let (effects, commands) = bar.handle_event(&ctx, &event);
        assert!(effects.is_empty());
        assert!(commands.is_empty());
    }

    #[test]
    fn status_bar_component_handle_effect_stores_info() {
        let mut bar = StatusBar::new();
        let info = StatusInfo {
            profile: "fast".to_string(),
            permission_mode: "readonly".to_string(),
            session_count: 5,
            hint: "Ctrl+C quit".to_string(),
            error: Some("oops".to_string()),
        };
        bar.handle_effect(&CrossPanelEffect::SetStatus(info.clone()));
        assert_eq!(bar.info.profile, "fast");
        assert_eq!(bar.info.permission_mode, "readonly");
        assert_eq!(bar.info.session_count, 5);
        assert_eq!(bar.info.error, Some("oops".to_string()));
    }

    #[test]
    fn status_bar_component_not_focused_by_default() {
        let bar = StatusBar::new();
        assert!(!bar.focused());
    }
}

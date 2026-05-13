//! StatusBar component — a read-only single-line bar at the bottom of the TUI.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::{Command, Component, CrossPanelEffect, EventContext, StatusInfo};

// `ContextUsage` is reachable through the `StatusInfo.context_usage` field
// declaration in `mod.rs` (no explicit import needed in this module);
// `ContextSource` is needed for the per-source breakdown helper below.
use agent_core::context_types::ContextSource;

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
/// MCP server count, a hint, and optional error text.
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
                mcp_server_count: 0,
                hint: String::new(),
                error: None,
                context_usage: None,
                compacting: false,
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
/// [ profile ] [ mode ] sessions: N  [MCP:N↑]  hint text  error!
/// ```
///
/// - **profile** — cyan background, bold
/// - **permission mode** — yellow background, bold
/// - **session count** — default style
/// - **MCP server count** — magenta, shown only if > 0
/// - **hint** — dim
/// - **error** (if present) — red foreground, bold
pub fn render_status_bar(area: Rect, frame: &mut Frame, info: &StatusInfo) {
    // P3: when we have observed at least one ContextAssembled event, switch
    // to the dedicated context-meter line. The legacy renderer below remains
    // the fallback for the cold-start case (no usage yet).
    if info.context_usage.is_some() {
        let line_text = render_context_line_string(info, area.width);
        frame.render_widget(Paragraph::new(Line::from(Span::raw(line_text))), area);
        return;
    }

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

    // MCP server count (only shown if > 0)
    if info.mcp_server_count > 0 {
        spans.push(Span::styled(
            format!("MCP:{}↑", info.mcp_server_count),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("  "));
    }

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
// P3: Context-meter status line
// ---------------------------------------------------------------------------

/// Format a token count as `1.2k` for >=1000, otherwise the raw number.
fn fmt_tokens(n: u64) -> String {
    if n >= 1_000 {
        format!("{:.1}k", (n as f64) / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Compact form of `fmt_tokens` for per-source breakdown chips: `12k` (no decimal).
fn fmt_short(n: u64) -> String {
    if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

/// Map a [`ContextSource`] to a 3-5 char chip label for the breakdown line.
fn source_short_label(source: &ContextSource) -> &'static str {
    match source {
        ContextSource::System => "sys",
        ContextSource::ToolDefinitions => "tools",
        ContextSource::Request => "req",
        ContextSource::Memory => "mem",
        ContextSource::History => "hist",
        ContextSource::ToolResult => "tres",
        ContextSource::SelectedFile => "file",
        ContextSource::CompactionSummary => "csum",
        ContextSource::Skill => "skill",
        ContextSource::ProjectInstruction => "proj",
    }
}

/// Render a single status line including the context-meter info as a plain
/// `String` (so unit tests can assert on the text without going through
/// ratatui rendering).
///
/// Layout:
/// - Always: `profile: <name>  perm: <mode>`
/// - When `usage.is_some()`:
///   - `width >= 100`: long form `ctx: <tot>/<bud>[ ⚠]  <chip1> <n1> <chip2> <n2> …`
///   - `width <  100`: short form `ctx: <tot>/<bud> (<pct>%)[ ⚠]`
/// - When `usage.is_none()`: `ctx: -`
/// - When `compacting`: appends `compacting…`
pub fn render_context_line_string(info: &StatusInfo, width: u16) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!("profile: {}", info.profile));
    parts.push(format!("perm: {}", info.permission_mode));

    match &info.context_usage {
        Some(u) => {
            let pct = if u.budget_tokens == 0 {
                0
            } else {
                (((u.total_tokens as f64) / (u.budget_tokens as f64)) * 100.0).round() as u64
            };
            // Warning glyph at >=70%; the GUI surfaces an additional badge for
            // the >=85% err tier — the TUI keeps a single tier here to stay
            // readable on a one-row status bar.
            let warn = if pct >= 70 { " ⚠" } else { "" };

            if width >= 100 {
                parts.push(format!(
                    "ctx: {}/{}{}",
                    fmt_tokens(u.total_tokens),
                    fmt_tokens(u.budget_tokens),
                    warn
                ));
                let mut breakdown = String::new();
                for (source, tokens) in &u.by_source {
                    breakdown.push_str(&format!(
                        " {} {}",
                        source_short_label(source),
                        fmt_short(*tokens)
                    ));
                }
                parts.push(breakdown.trim_start().to_string());
            } else {
                parts.push(format!(
                    "ctx: {}/{} ({}%){}",
                    fmt_tokens(u.total_tokens),
                    fmt_tokens(u.budget_tokens),
                    pct,
                    warn
                ));
            }
        }
        None => parts.push("ctx: -".into()),
    }

    if info.compacting {
        parts.push("compacting…".into());
    }

    parts.join("  ")
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
            mcp_server_count: 2,
            hint: "Alt+Q quit".to_string(),
            error: None,
            context_usage: None,
            compacting: false,
        };

        let mut terminal = test_terminal(80, 1);
        terminal
            .draw(|frame| {
                render_status_bar(frame.area(), frame, &info);
            })
            .expect("render should not panic");
    }

    #[test]
    fn status_bar_renders_with_mcp_count_zero() {
        let info = StatusInfo {
            profile: "fast".to_string(),
            permission_mode: "suggest".to_string(),
            session_count: 3,
            mcp_server_count: 0,
            hint: "Alt+Q quit".to_string(),
            error: None,
            context_usage: None,
            compacting: false,
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
            mcp_server_count: 0,
            hint: "F1 help".to_string(),
            error: Some("connection lost".to_string()),
            context_usage: None,
            compacting: false,
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
            mcp_server_count: 0,
            hint: String::new(),
            error: None,
            context_usage: None,
            compacting: false,
        };
        assert_eq!(info.permission_mode_label(), "agent");
    }

    #[test]
    fn status_bar_component_handle_event_returns_empty() {
        let mut bar = StatusBar::new();
        static WS_ID: std::sync::OnceLock<agent_core::WorkspaceId> = std::sync::OnceLock::new();
        static SID: std::sync::OnceLock<Option<agent_core::SessionId>> = std::sync::OnceLock::new();
        let ws_id = WS_ID.get_or_init(agent_core::WorkspaceId::new);
        let sid = SID.get_or_init(|| None);
        let ctx = EventContext {
            focus: super::super::FocusTarget::Chat,
            current_session: &agent_core::projection::SessionProjection::default(),
            sessions: &[],
            model_profile: "fast",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: false,
            workspace_id: ws_id,
            current_session_id: sid,
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
            mcp_server_count: 3,
            hint: "Ctrl+C quit".to_string(),
            error: Some("oops".to_string()),
            context_usage: None,
            compacting: false,
        };
        bar.handle_effect(&CrossPanelEffect::SetStatus(info.clone()));
        assert_eq!(bar.info.profile, "fast");
        assert_eq!(bar.info.permission_mode, "readonly");
        assert_eq!(bar.info.session_count, 5);
        assert_eq!(bar.info.mcp_server_count, 3);
        assert_eq!(bar.info.error, Some("oops".to_string()));
    }

    #[test]
    fn status_bar_component_not_focused_by_default() {
        let bar = StatusBar::new();
        assert!(!bar.focused());
    }
}

#[cfg(test)]
mod context_line_tests {
    use super::*;
    use agent_core::context_types::{ContextSource, ContextUsage};

    fn usage(total: u64, budget: u64) -> ContextUsage {
        ContextUsage {
            total_tokens: total,
            budget_tokens: budget,
            context_window: budget + 20_000,
            output_reservation: 20_000,
            by_source: vec![
                (ContextSource::System, 2_000),
                (ContextSource::ToolDefinitions, 22_000),
                (ContextSource::Memory, 9_000),
                (ContextSource::History, 64_000),
                (ContextSource::ToolResult, 13_000),
            ],
            estimator: "cl100k_base".to_string(),
            corrected_by_real_usage: false,
        }
    }

    fn make_info(usage_opt: Option<ContextUsage>, compacting: bool) -> StatusInfo {
        StatusInfo {
            profile: "fast".into(),
            permission_mode: "suggest".into(),
            session_count: 1,
            mcp_server_count: 0,
            hint: String::new(),
            error: None,
            context_usage: usage_opt,
            compacting,
        }
    }

    #[test]
    fn render_context_line_long_form_under_wide_terminal() {
        let info = make_info(Some(usage(110_000, 180_000)), false);
        let rendered = render_context_line_string(&info, 120);
        assert!(rendered.contains("profile: fast"), "got: {rendered}");
        assert!(rendered.contains("perm: suggest"), "got: {rendered}");
        assert!(rendered.contains("ctx: 110.0k/180.0k"), "got: {rendered}");
        assert!(rendered.contains("sys 2k"), "got: {rendered}");
        assert!(rendered.contains("hist 64k"), "got: {rendered}");
    }

    #[test]
    fn render_context_line_short_form_under_narrow_terminal() {
        let info = make_info(Some(usage(152_000, 200_000)), false);
        let rendered = render_context_line_string(&info, 60);
        assert!(rendered.contains("ctx: 152.0k/200.0k"), "got: {rendered}");
        assert!(rendered.contains("(76%)"), "got: {rendered}");
        // Short form does NOT include per-source breakdown.
        assert!(!rendered.contains("sys 2k"), "got: {rendered}");
    }

    #[test]
    fn render_context_line_warn_glyph_at_70_pct() {
        let info = make_info(Some(usage(140_000, 180_000)), false); // ≈78%
        let rendered = render_context_line_string(&info, 60);
        assert!(rendered.contains('⚠'), "got: {rendered}");
    }

    #[test]
    fn render_context_line_shows_compacting_indicator() {
        let info = make_info(Some(usage(50_000, 180_000)), true);
        let rendered = render_context_line_string(&info, 120);
        assert!(rendered.contains("compacting"), "got: {rendered}");
    }

    #[test]
    fn render_context_line_handles_no_usage_gracefully() {
        let info = make_info(None, false);
        let rendered = render_context_line_string(&info, 120);
        assert!(rendered.contains("profile: fast"), "got: {rendered}");
        assert!(rendered.contains("ctx: -"), "got: {rendered}");
    }
}

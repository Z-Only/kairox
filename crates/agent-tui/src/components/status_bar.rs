//! StatusBar component — a read-only single-line bar at the bottom of the TUI.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
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

    /// Return the next permission mode in the cycle order.
    ///
    /// Order: ReadOnly → Suggest → Agent → Autonomous → Interactive → ReadOnly.
    fn next(&self) -> agent_tools::PermissionMode;
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

    fn next(&self) -> agent_tools::PermissionMode {
        match self {
            agent_tools::PermissionMode::ReadOnly => agent_tools::PermissionMode::Suggest,
            agent_tools::PermissionMode::Suggest => agent_tools::PermissionMode::Agent,
            agent_tools::PermissionMode::Agent => agent_tools::PermissionMode::Autonomous,
            agent_tools::PermissionMode::Autonomous => agent_tools::PermissionMode::Interactive,
            agent_tools::PermissionMode::Interactive => agent_tools::PermissionMode::ReadOnly,
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
    context_details_visible: bool,
    notifications: Vec<String>,
}

impl StatusBar {
    const NOTIFICATION_LOG_LIMIT: usize = 100;

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

fn render_context_details_overlay(status_area: Rect, frame: &mut Frame, info: &StatusInfo) {
    let detail_lines = render_context_details_lines(info);
    let Some(area) = context_details_overlay_area(status_area, detail_lines.len()) else {
        return;
    };

    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" Context Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = detail_lines.into_iter().map(Line::from).collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn context_details_overlay_area(status_area: Rect, line_count: usize) -> Option<Rect> {
    if status_area.width == 0 || status_area.y < 3 {
        return None;
    }

    let desired_height = (line_count as u16).saturating_add(2);
    let height = desired_height.min(status_area.y).max(3);
    let width = status_area.width.min(78);
    let x = status_area.x + status_area.width.saturating_sub(width);
    let y = status_area.y.saturating_sub(height);

    Some(Rect {
        x,
        y,
        width,
        height,
    })
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
#[allow(dead_code)]
pub fn render_status_bar(area: Rect, frame: &mut Frame, info: &StatusInfo) {
    render_status_bar_with_notification(area, frame, info, None);
}

fn render_status_bar_with_notification(
    area: Rect,
    frame: &mut Frame,
    info: &StatusInfo,
    notification: Option<&str>,
) {
    // P3: when we have observed at least one ContextAssembled event, switch
    // to the dedicated context-meter line. The legacy renderer below remains
    // the fallback for the cold-start case (no usage yet).
    if info.context_usage.is_some() {
        let mut line_text = render_context_line_string(info, area.width);
        if let Some(notification) = notification.filter(|value| !value.is_empty()) {
            line_text.push_str("  status: ");
            line_text.push_str(notification);
        }
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

    if let Some(notification) = notification.filter(|value| !value.is_empty()) {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            notification,
            status_notification_style(notification),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}

fn status_notification_style(message: &str) -> Style {
    if message.starts_with('[') && message.contains("error") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    }
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

fn percent_of(tokens: u64, budget_tokens: u64) -> u64 {
    if budget_tokens == 0 {
        0
    } else {
        (((tokens as f64) / (budget_tokens as f64)) * 100.0).round() as u64
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

fn source_label(source: &ContextSource) -> &'static str {
    match source {
        ContextSource::System => "System",
        ContextSource::ToolDefinitions => "Tools",
        ContextSource::Request => "Request",
        ContextSource::Memory => "Memory",
        ContextSource::History => "History",
        ContextSource::ToolResult => "Tool result",
        ContextSource::SelectedFile => "Selected file",
        ContextSource::CompactionSummary => "Compaction summary",
        ContextSource::Skill => "Skill",
        ContextSource::ProjectInstruction => "Project instructions",
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

pub fn render_context_details_lines(info: &StatusInfo) -> Vec<String> {
    let Some(usage) = &info.context_usage else {
        return vec![
            "No context usage yet".to_string(),
            format!(
                "Compaction: {}",
                if info.compacting { "running" } else { "idle" }
            ),
            "[Esc] close".to_string(),
        ];
    };

    let pct = percent_of(usage.total_tokens, usage.budget_tokens);
    let mut lines = vec![
        format!(
            "Used: {} / {} ({}%)",
            fmt_tokens(usage.total_tokens),
            fmt_tokens(usage.budget_tokens),
            pct
        ),
        format!("Context window: {}", fmt_tokens(usage.context_window)),
        format!(
            "Reserved for response: {}",
            fmt_tokens(usage.output_reservation)
        ),
        format!(
            "Compaction: {}",
            if info.compacting { "running" } else { "idle" }
        ),
        "Source breakdown:".to_string(),
    ];

    for (source, tokens) in &usage.by_source {
        lines.push(format!(
            "  {:<20} {:>7} {:>3}%",
            source_label(source),
            fmt_tokens(*tokens),
            percent_of(*tokens, usage.budget_tokens)
        ));
    }

    let compact_hint = if info.compacting {
        "[c] compacting...  [Esc] close"
    } else {
        "[c] compact now  [Esc] close"
    };
    lines.push(compact_hint.to_string());
    lines
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

    if info.context_usage.is_some() {
        parts.push("Alt+C details".into());
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
    fn cycles_permission_mode() {
        use agent_tools::PermissionMode;
        use PermissionModeExt;

        assert_eq!(PermissionMode::ReadOnly.next(), PermissionMode::Suggest);
        assert_eq!(PermissionMode::Suggest.next(), PermissionMode::Agent);
        assert_eq!(PermissionMode::Agent.next(), PermissionMode::Autonomous);
        assert_eq!(
            PermissionMode::Autonomous.next(),
            PermissionMode::Interactive
        );
        assert_eq!(PermissionMode::Interactive.next(), PermissionMode::ReadOnly);
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
            projects: &[],
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

    #[test]
    fn status_bar_keeps_bounded_notification_log() {
        let mut bar = StatusBar::new();

        for index in 0..105 {
            bar.push_notification(format!("status {index}"));
        }

        assert_eq!(bar.notifications.len(), StatusBar::NOTIFICATION_LOG_LIMIT);
        assert_eq!(bar.latest_notification(), Some("status 104"));
        assert_eq!(
            bar.notifications.first().map(String::as_str),
            Some("status 5")
        );
    }
}

#[cfg(test)]
mod context_line_tests {
    use super::*;
    use agent_core::context_types::{ContextSource, ContextUsage};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

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

    #[test]
    fn render_context_details_includes_breakdown_and_reservation() {
        let info = make_info(Some(usage(110_000, 180_000)), false);
        let rendered = render_context_details_lines(&info)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            rendered.contains("Used: 110.0k / 180.0k (61%)"),
            "got: {rendered}"
        );
        assert!(rendered.contains("System"), "got: {rendered}");
        assert!(rendered.contains("2.0k"), "got: {rendered}");
        assert!(rendered.contains("History"), "got: {rendered}");
        assert!(rendered.contains("64.0k"), "got: {rendered}");
        assert!(
            rendered.contains("Reserved for response: 20.0k"),
            "got: {rendered}"
        );
        assert!(rendered.contains("Compaction: idle"), "got: {rendered}");
    }

    #[test]
    fn context_details_c_emits_compact_session_command() {
        let workspace_id = agent_core::WorkspaceId::from_string("wrk_test".into());
        let session_id = agent_core::SessionId::from_string("ses_test".into());
        let current_session_id = Some(session_id.clone());
        let projection = agent_core::projection::SessionProjection::default();
        let ctx = EventContext {
            focus: super::super::FocusTarget::Chat,
            current_session: &projection,
            projects: &[],
            sessions: &[],
            model_profile: "fast",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: false,
            workspace_id: &workspace_id,
            current_session_id: &current_session_id,
        };
        let event = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        let mut bar = StatusBar::new();
        bar.handle_effect(&CrossPanelEffect::SetStatus(make_info(
            Some(usage(110_000, 180_000)),
            false,
        )));
        bar.toggle_context_details();

        let (_effects, commands) = bar.handle_event(&ctx, &event);

        assert_eq!(
            commands,
            vec![Command::CompactSession {
                workspace_id,
                session_id,
            }]
        );
    }
}

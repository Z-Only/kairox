//! Unit tests for the status bar — moved verbatim from the original
//! single-file `status_bar.rs`. Behaviour and assertions are unchanged.

use super::context_line::{render_context_details_lines, render_context_line_string};
use super::render::render_status_bar;
use super::StatusBar;

use crate::components::{Command, CrossPanelEffect, EventContext, FocusTarget, StatusInfo};

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
        approval_policy: String::new(),
        sandbox_policy: String::new(),
        session_count: 3,
        mcp_server_count: 2,
        session_metadata: Vec::new(),
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
        approval_policy: String::new(),
        sandbox_policy: String::new(),
        session_count: 3,
        mcp_server_count: 0,
        session_metadata: Vec::new(),
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
        approval_policy: String::new(),
        sandbox_policy: String::new(),
        session_count: 1,
        mcp_server_count: 0,
        session_metadata: Vec::new(),
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
fn status_bar_component_handle_event_returns_empty() {
    use crate::components::Component;

    let mut bar = StatusBar::new();
    static WS_ID: std::sync::OnceLock<agent_core::WorkspaceId> = std::sync::OnceLock::new();
    static SID: std::sync::OnceLock<Option<agent_core::SessionId>> = std::sync::OnceLock::new();
    let ws_id = WS_ID.get_or_init(agent_core::WorkspaceId::new);
    let sid = SID.get_or_init(|| None);
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &agent_core::projection::SessionProjection::default(),
        projects: &[],
        sessions: &[],
        model_profile: "fast",
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
    use crate::components::Component;

    let mut bar = StatusBar::new();
    let info = StatusInfo {
        profile: "fast".to_string(),
        approval_policy: String::new(),
        sandbox_policy: String::new(),
        session_count: 5,
        mcp_server_count: 3,
        session_metadata: Vec::new(),
        hint: "Ctrl+C quit".to_string(),
        error: Some("oops".to_string()),
        context_usage: None,
        compacting: false,
    };
    bar.handle_effect(&CrossPanelEffect::SetStatus(info.clone()));
    assert_eq!(bar.info.profile, "fast");
    assert_eq!(bar.info.session_count, 5);
    assert_eq!(bar.info.mcp_server_count, 3);
    assert_eq!(bar.info.error, Some("oops".to_string()));
}

#[test]
fn status_bar_component_not_focused_by_default() {
    use crate::components::Component;

    let bar = StatusBar::new();
    assert!(!bar.focused());
}

#[test]
fn status_bar_keeps_bounded_notification_log() {
    let mut bar = StatusBar::new();

    for index in 0..105 {
        bar.push_notification(format!("status {index}"));
    }

    assert_eq!(
        bar.notifications.len(),
        super::types::NOTIFICATION_LOG_LIMIT
    );
    assert_eq!(bar.latest_notification(), Some("status 104"));
    assert_eq!(
        bar.notifications.first().map(String::as_str),
        Some("status 5")
    );
}

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
            approval_policy: String::new(),
            sandbox_policy: String::new(),
            session_count: 1,
            mcp_server_count: 0,
            session_metadata: Vec::new(),
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
        use crate::components::Component;

        let workspace_id = agent_core::WorkspaceId::from_string("wrk_test".into());
        let session_id = agent_core::SessionId::from_string("ses_test".into());
        let current_session_id = Some(session_id.clone());
        let projection = agent_core::projection::SessionProjection::default();
        let ctx = EventContext {
            focus: FocusTarget::Chat,
            current_session: &projection,
            projects: &[],
            sessions: &[],
            model_profile: "fast",
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

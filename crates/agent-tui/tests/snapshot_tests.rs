//! Snapshot tests for rendered UI — chat panel rendering via ratatui TestBackend + insta.

use agent_core::facade::TaskGraphSnapshot;
use agent_core::projection::{ProjectedMessage, ProjectedRole, SessionProjection};
use agent_core::WorkspaceId;
use agent_tui::app::App;
use agent_tui::components::chat::render_messages;
use agent_tui::components::FocusTarget;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

mod support;

use support::render::render_app;

#[test]
fn chat_panel_renders_user_and_assistant_messages() {
    let projection = SessionProjection {
        messages: vec![
            ProjectedMessage {
                role: ProjectedRole::User,
                content: "What is the meaning of life?".to_string(),
            },
            ProjectedMessage {
                role: ProjectedRole::Assistant,
                content: "42".to_string(),
            },
        ],
        task_titles: vec![],
        task_graph: TaskGraphSnapshot::default(),
        token_stream: String::new(),
        cancelled: false,
        last_context_usage: None,
        model_limits: None,
        compaction: agent_core::projection::CompactionStatus::Idle,
    };

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_messages(frame.area(), frame, &projection);
        })
        .unwrap();

    insta::assert_snapshot!(terminal.backend().to_string());
}

#[test]
fn chat_panel_renders_streaming_token_with_cursor() {
    let projection = SessionProjection {
        messages: vec![ProjectedMessage {
            role: ProjectedRole::User,
            content: "tell me a story".to_string(),
        }],
        task_titles: vec![],
        task_graph: TaskGraphSnapshot::default(),
        token_stream: "Once upon a time".to_string(),
        cancelled: false,
        last_context_usage: None,
        model_limits: None,
        compaction: agent_core::projection::CompactionStatus::Idle,
    };

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_messages(frame.area(), frame, &projection);
        })
        .unwrap();

    let output = terminal.backend().to_string();
    // Verify the block cursor character appears
    assert!(
        output.contains('▌'),
        "streaming output should contain ▌ cursor, got:\n{}",
        output
    );

    insta::assert_snapshot!(output);
}

#[test]
fn help_overlay_renders_keybinding_snapshot() {
    let mut app = App::new("test", WorkspaceId::from_string("wrk_test".into()));
    app.handle_crossterm_event(&crossterm::event::Event::Key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::F(1),
            crossterm::event::KeyModifiers::NONE,
        ),
    ));

    let output = render_app(&mut app, 100, 28);

    assert!(output.contains("Help / Keybindings"));
    assert!(output.contains("Global shortcuts"));
    assert!(output.contains("Common commands"));
    insta::assert_snapshot!(output);
}

#[test]
fn help_overlay_content_changes_with_current_focus() {
    let mut app = App::new("test", WorkspaceId::from_string("wrk_test".into()));

    app.handle_crossterm_event(&crossterm::event::Event::Key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::F(1),
            crossterm::event::KeyModifiers::NONE,
        ),
    ));
    let chat_help = render_app(&mut app, 100, 28);
    assert!(chat_help.contains("Current focus: Chat composer"));
    assert!(chat_help.contains("Ctrl+Enter"));

    app.handle_crossterm_event(&crossterm::event::Event::Key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::F(1),
            crossterm::event::KeyModifiers::NONE,
        ),
    ));
    app.state.focus_manager.set(FocusTarget::Trace);
    app.sync_component_focus();
    app.handle_crossterm_event(&crossterm::event::Event::Key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::F(1),
            crossterm::event::KeyModifiers::NONE,
        ),
    ));
    let trace_help = render_app(&mut app, 100, 28);

    assert!(trace_help.contains("Current focus: Trace panel"));
    assert!(trace_help.contains("F5"));
    assert_ne!(chat_help, trace_help);
}

#[test]
fn help_overlay_content_changes_with_current_overlay() {
    let mut app = App::new("test", WorkspaceId::from_string("wrk_test".into()));
    app.dispatch_effects(vec![
        agent_tui::components::CrossPanelEffect::ShowCommandPalette,
    ]);

    app.handle_crossterm_event(&crossterm::event::Event::Key(
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::F(1),
            crossterm::event::KeyModifiers::NONE,
        ),
    ));
    let overlay_help = render_app(&mut app, 100, 28);

    assert!(overlay_help.contains("Current overlay: Command palette"));
    assert!(overlay_help.contains("Enter run selected"));
}

#[test]
fn chat_panel_renders_cancelled_marker() {
    let projection = SessionProjection {
        messages: vec![ProjectedMessage {
            role: ProjectedRole::User,
            content: "do something dangerous".to_string(),
        }],
        task_titles: vec![],
        task_graph: TaskGraphSnapshot::default(),
        token_stream: String::new(),
        cancelled: true,
        last_context_usage: None,
        model_limits: None,
        compaction: agent_core::projection::CompactionStatus::Idle,
    };

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_messages(frame.area(), frame, &projection);
        })
        .unwrap();

    let output = terminal.backend().to_string();
    // Verify the cancelled marker appears
    assert!(
        output.contains("[cancelled]"),
        "cancelled output should contain [cancelled], got:\n{}",
        output
    );

    insta::assert_snapshot!(output);
}

//! Full-app render coverage for core TUI surfaces.
//!
//! These tests render the composed `App` into a ratatui `TestBackend`. They
//! intentionally assert user-visible text instead of internal component state.

use agent_core::facade::{TaskGraphSnapshot, TaskSnapshot};
use agent_core::projection::{ProjectedMessage, ProjectedRole};
use agent_core::{AgentRole, ProjectSessionVisibility, SessionId, TaskId, TaskState, WorkspaceId};
use agent_tools::PermissionMode;
use agent_tui::app::App;
use agent_tui::components::trace::RightPanelTab;
use agent_tui::components::{FocusTarget, QueuedMessage, SessionInfo, SessionState};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_app(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal should be created");
    terminal
        .draw(|frame| app.render(frame))
        .expect("app should render");
    terminal.backend().to_string()
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(code, modifiers))
}

fn seeded_app() -> App {
    let workspace_id = WorkspaceId::from_string("wrk_full_app_render".into());
    let session_id = SessionId::from_string("ses_full_app_render".into());
    let mut app = App::new("fake", PermissionMode::Suggest, workspace_id);
    app.current_session_id = Some(session_id.clone());
    app.state.sessions = vec![SessionInfo {
        id: session_id,
        title: "Full app smoke".into(),
        model_profile: "fake".into(),
        state: SessionState::Active,
        pinned: false,
        archived: false,
        project_id: None,
        worktree_path: None,
        branch: None,
        visibility: Some(ProjectSessionVisibility::Visible),
    }];
    app.state.current_session.messages = vec![
        ProjectedMessage {
            role: ProjectedRole::User,
            content: "hello tui".into(),
        },
        ProjectedMessage {
            role: ProjectedRole::Assistant,
            content: "render ok".into(),
        },
    ];
    app.state.current_session.task_graph = TaskGraphSnapshot {
        tasks: vec![TaskSnapshot {
            id: TaskId::from_string("task_full_app".into()),
            title: "Render core shell".into(),
            role: AgentRole::Worker,
            state: TaskState::Running,
            dependencies: Vec::new(),
            error: None,
            retry_count: 0,
            max_retries: 1,
            assigned_agent_id: None,
            failure_reason: None,
        }],
    };
    app.state.sidebar_right_visible = true;
    app.chat.input_content = "draft text".into();
    app.chat.message_queue.push(QueuedMessage {
        content: "queued follow-up".into(),
        attachments: Vec::new(),
    });
    app.trace.active_tab = RightPanelTab::Tasks;
    app.sync_status_bar();
    app.sync_component_focus();
    app
}

#[test]
fn full_app_renders_core_shell_regions_without_hiding_composer() {
    let mut app = seeded_app();

    let output = render_app(&mut app, 140, 36);

    assert!(output.contains("Projects / Sessions"), "{output}");
    assert!(output.contains("Full app smoke"), "{output}");
    assert!(output.contains("You:"), "{output}");
    assert!(output.contains("hello tui"), "{output}");
    assert!(output.contains("Agent:"), "{output}");
    assert!(output.contains("render ok"), "{output}");
    assert!(output.contains("[Tasks]"), "{output}");
    assert!(output.contains("Render core shell"), "{output}");
    assert!(output.contains("queued follow-up"), "{output}");
    assert!(
        output.contains("> │ draft text"),
        "composer prompt and draft should be visible:\n{output}"
    );
    assert!(output.contains(" fake "), "{output}");
    assert!(output.contains(" suggest "), "{output}");
}

#[test]
fn full_app_renders_help_and_command_palette_overlays_above_shell() {
    let mut app = seeded_app();

    app.handle_crossterm_event(&key(KeyCode::Char('p'), KeyModifiers::CONTROL));
    let palette = render_app(&mut app, 140, 36);
    assert!(palette.contains("Command Palette"), "{palette}");
    assert!(palette.contains("MCP: open manager"), "{palette}");
    assert!(palette.contains("Models: open selector"), "{palette}");

    app.handle_crossterm_event(&key(KeyCode::Esc, KeyModifiers::NONE));
    app.handle_crossterm_event(&key(KeyCode::F(1), KeyModifiers::NONE));
    let help = render_app(&mut app, 140, 36);
    assert!(help.contains("Help / Keybindings"), "{help}");
    assert!(help.contains("Current focus: Chat composer"), "{help}");
    assert!(help.contains("Global shortcuts"), "{help}");
}

#[test]
fn full_app_focus_and_sidebar_shortcuts_update_visible_shell() {
    let mut app = seeded_app();

    app.handle_crossterm_event(&key(KeyCode::Char('s'), KeyModifiers::ALT));
    app.handle_crossterm_event(&key(KeyCode::Char('t'), KeyModifiers::ALT));
    let compact = render_app(&mut app, 100, 28);
    assert!(!compact.contains("Projects / Sessions"), "{compact}");
    assert!(!compact.contains("[Tasks]"), "{compact}");
    assert!(compact.contains("> │ draft text"), "{compact}");

    app.handle_crossterm_event(&key(KeyCode::Char('s'), KeyModifiers::ALT));
    app.handle_crossterm_event(&key(KeyCode::Char('t'), KeyModifiers::ALT));
    app.handle_crossterm_event(&key(KeyCode::Tab, KeyModifiers::NONE));
    let restored = render_app(&mut app, 140, 36);
    assert!(restored.contains("Projects / Sessions"), "{restored}");
    assert!(restored.contains("[Tasks]"), "{restored}");
    assert_eq!(app.state.focus_manager.current(), FocusTarget::Sessions);
}

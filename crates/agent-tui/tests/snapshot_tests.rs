//! Snapshot tests for rendered UI — chat panel rendering via ratatui TestBackend + insta.

use agent_core::projection::{ProjectedMessage, ProjectedRole, SessionProjection};
use agent_tui::components::chat::render_messages;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

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
        token_stream: String::new(),
        cancelled: false,
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
        token_stream: "Once upon a time".to_string(),
        cancelled: false,
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
fn chat_panel_renders_cancelled_marker() {
    let projection = SessionProjection {
        messages: vec![ProjectedMessage {
            role: ProjectedRole::User,
            content: "do something dangerous".to_string(),
        }],
        task_titles: vec![],
        token_stream: String::new(),
        cancelled: true,
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

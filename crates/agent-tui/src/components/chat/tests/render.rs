//! Pure-render helper coverage: `render_messages` for steady and
//! streaming/cancelled projections, plus the queued-message strip.

use super::super::*;
use crate::components::QueuedMessage;

#[test]
fn render_messages_basic() {
    use agent_core::facade::TaskGraphSnapshot;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let projection = agent_core::projection::SessionProjection {
        messages: vec![
            agent_core::projection::ProjectedMessage {
                role: agent_core::projection::ProjectedRole::User,
                content: "hello".to_string(),
            },
            agent_core::projection::ProjectedMessage {
                role: agent_core::projection::ProjectedRole::Assistant,
                content: "world".to_string(),
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

    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_messages(frame.area(), frame, &projection);
        })
        .expect("render_messages should not panic");
}

#[test]
fn render_messages_with_streaming_and_cancelled() {
    use agent_core::facade::TaskGraphSnapshot;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let projection = agent_core::projection::SessionProjection {
        messages: vec![agent_core::projection::ProjectedMessage {
            role: agent_core::projection::ProjectedRole::User,
            content: "go".to_string(),
        }],
        task_titles: vec![],
        task_graph: TaskGraphSnapshot::default(),
        token_stream: "thinking".to_string(),
        cancelled: true,
        last_context_usage: None,
        model_limits: None,
        compaction: agent_core::projection::CompactionStatus::Idle,
    };

    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_messages(frame.area(), frame, &projection);
        })
        .expect("render_messages should not panic");
}

#[test]
fn queue_strip_renders_multiple_messages_and_selected_row() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let queue = ["first", "second", "third"]
        .into_iter()
        .map(|content| QueuedMessage {
            content: content.to_string(),
            attachments: Vec::new(),
        })
        .collect::<Vec<_>>();
    let backend = TestBackend::new(80, 5);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_queue_strip(frame.area(), frame, &queue, Some(1));
        })
        .expect("render_queue_strip should not panic");

    let output = terminal.backend().to_string();
    assert!(output.contains("Q1 first"), "{output}");
    assert!(output.contains("> Q2 second"), "{output}");
    assert!(output.contains("Q3 third"), "{output}");
    assert!(output.contains("3 queued"), "{output}");
}

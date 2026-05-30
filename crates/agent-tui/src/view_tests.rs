use super::*;
use agent_core::facade::TaskGraphSnapshot;
use agent_core::projection::{ProjectedMessage, ProjectedRole, SessionProjection};

#[test]
fn renders_chat_messages_from_projection() {
    let projection = SessionProjection {
        messages: vec![
            ProjectedMessage {
                role: ProjectedRole::User,
                content: "hi".into(),
            },
            ProjectedMessage {
                role: ProjectedRole::Assistant,
                content: "hello".into(),
            },
        ],
        task_titles: vec!["Session using fake".into()],
        task_graph: TaskGraphSnapshot::default(),
        token_stream: String::new(),
        cancelled: false,
        last_context_usage: None,
        model_limits: None,
        compaction: agent_core::projection::CompactionStatus::Idle,
    };

    assert_eq!(render_lines(&projection), vec!["You: hi", "Agent: hello"]);
}

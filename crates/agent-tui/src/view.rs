//! Legacy text-only view helper.
//!
//! **Note:** This module is superseded by the component renderers in
//! [`crate::components::chat::render_messages`], [`crate::components::sessions`],
//! [`crate::components::trace`], etc. It is retained for backwards compatibility
//! and simple string-based tests only.

use agent_core::projection::{ProjectedRole, SessionProjection};

#[allow(dead_code)]
pub fn render_lines(projection: &SessionProjection) -> Vec<String> {
    projection
        .messages
        .iter()
        .map(|message| match message.role {
            ProjectedRole::User => format!("You: {}", message.content),
            ProjectedRole::Assistant => format!("Agent: {}", message.content),
        })
        .collect()
}

#[cfg(test)]
mod tests {
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
        };

        assert_eq!(render_lines(&projection), vec!["You: hi", "Agent: hello"]);
    }
}

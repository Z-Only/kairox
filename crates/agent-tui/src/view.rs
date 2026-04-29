use agent_core::projection::{ProjectedRole, SessionProjection};

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
            token_stream: String::new(),
            cancelled: false,
        };

        assert_eq!(render_lines(&projection), vec!["You: hi", "Agent: hello"]);
    }
}

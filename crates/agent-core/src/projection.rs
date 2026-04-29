use crate::events::{DomainEvent, EventPayload};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionProjection {
    pub messages: Vec<ProjectedMessage>,
    pub task_titles: Vec<String>,
    pub token_stream: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedMessage {
    pub role: ProjectedRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectedRole {
    User,
    Assistant,
}

impl SessionProjection {
    pub fn apply(&mut self, event: &DomainEvent) {
        match &event.payload {
            EventPayload::UserMessageAdded { content, .. } => {
                self.messages.push(ProjectedMessage {
                    role: ProjectedRole::User,
                    content: content.clone(),
                });
            }
            EventPayload::ModelTokenDelta { delta } => self.token_stream.push_str(delta),
            EventPayload::AssistantMessageCompleted { content, .. } => {
                self.messages.push(ProjectedMessage {
                    role: ProjectedRole::Assistant,
                    content: content.clone(),
                });
                self.token_stream.clear();
            }
            EventPayload::AgentTaskCreated { title, .. } => self.task_titles.push(title.clone()),
            EventPayload::SessionCancelled { .. } => self.cancelled = true,
            _ => {}
        }
    }

    pub fn from_events(events: &[DomainEvent]) -> Self {
        let mut projection = Self::default();
        for event in events {
            projection.apply(event);
        }
        projection
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentId, EventPayload, PrivacyClassification, SessionId, WorkspaceId};

    #[test]
    fn projects_user_and_assistant_messages() {
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();
        let events = vec![
            DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::UserMessageAdded {
                    message_id: "m1".into(),
                    content: "hello".into(),
                },
            ),
            DomainEvent::new(
                workspace_id,
                session_id,
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::AssistantMessageCompleted {
                    message_id: "m2".into(),
                    content: "hi".into(),
                },
            ),
        ];

        let projection = SessionProjection::from_events(&events);

        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].role, ProjectedRole::User);
        assert_eq!(projection.messages[1].content, "hi");
    }
}

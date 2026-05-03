use crate::events::{DomainEvent, EventPayload};
use crate::facade::TaskGraphSnapshot;
use crate::{TaskSnapshot, TaskState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SessionProjection {
    pub messages: Vec<ProjectedMessage>,
    pub task_titles: Vec<String>,
    pub task_graph: TaskGraphSnapshot,
    pub token_stream: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedMessage {
    pub role: ProjectedRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
            EventPayload::AgentTaskCreated {
                task_id,
                title,
                role,
                dependencies,
            } => {
                self.task_titles.push(title.clone());
                self.task_graph.tasks.push(TaskSnapshot {
                    id: task_id.clone(),
                    title: title.clone(),
                    role: *role,
                    state: TaskState::Pending,
                    dependencies: dependencies.clone(),
                    error: None,
                });
            }
            EventPayload::AgentTaskStarted { task_id } => {
                if let Some(t) = self.task_graph.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.state = TaskState::Running;
                }
            }
            EventPayload::AgentTaskCompleted { task_id } => {
                if let Some(t) = self.task_graph.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.state = TaskState::Completed;
                }
            }
            EventPayload::AgentTaskFailed { task_id, error } => {
                if let Some(t) = self.task_graph.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.state = TaskState::Failed;
                    t.error = Some(error.clone());
                }
            }
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
    use crate::{AgentId, AgentRole, EventPayload, PrivacyClassification, SessionId, WorkspaceId};

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

    #[test]
    fn serializes_projection_with_snake_case_roles() {
        let projection = SessionProjection {
            messages: vec![ProjectedMessage {
                role: ProjectedRole::Assistant,
                content: "hello".into(),
            }],
            task_titles: vec!["inspect repo".into()],
            task_graph: TaskGraphSnapshot::default(),
            token_stream: "hello".into(),
            cancelled: true,
        };

        let json = serde_json::to_value(&projection).unwrap();

        assert_eq!(json["messages"][0]["role"], "assistant");
        assert_eq!(json["messages"][0]["content"], "hello");
        assert_eq!(json["task_titles"][0], "inspect repo");
        assert_eq!(json["token_stream"], "hello");
        assert_eq!(json["cancelled"], true);

        let round_tripped: SessionProjection = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, projection);
    }

    #[test]
    fn projects_token_deltas_tasks_and_cancellation() {
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();
        let task_id = crate::TaskId::new();
        let events = vec![
            DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::ModelTokenDelta {
                    delta: "hel".into(),
                },
            ),
            DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::ModelTokenDelta { delta: "lo".into() },
            ),
            DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                AgentId::planner(),
                PrivacyClassification::MinimalTrace,
                EventPayload::AgentTaskCreated {
                    task_id,
                    title: "inspect repo".into(),
                    role: AgentRole::Planner,
                    dependencies: vec![],
                },
            ),
            DomainEvent::new(
                workspace_id,
                session_id,
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::SessionCancelled {
                    reason: "user stopped".into(),
                },
            ),
        ];

        let projection = SessionProjection::from_events(&events);

        assert_eq!(projection.token_stream, "hello");
        assert_eq!(projection.task_titles, vec!["inspect repo"]);
        assert!(projection.cancelled);
    }
}

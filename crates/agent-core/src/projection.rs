use crate::events::{DomainEvent, EventPayload};
use crate::facade::TaskGraphSnapshot;
use crate::{TaskSnapshot, TaskState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type")]
pub enum CompactionStatus {
    #[default]
    Idle,
    Running,
    Failed {
        error: String,
    },
}

/// Mirror of `agent_models::ModelLimits` so projections survive the
/// `agent-core` ← `agent-models` dependency boundary. The runtime converts
/// on the boundary; field shape is kept in lock-step manually.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectedModelLimits {
    pub context_window: u64,
    pub output_limit: u64,
    /// Snake-case `LimitSource` discriminant: "user_config" | "builtin_registry" | "runtime_probe" | "fallback".
    pub source: String,
}

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
                    retry_count: 0,
                    max_retries: 0,
                    assigned_agent_id: None,
                    failure_reason: None,
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
            EventPayload::SessionInitialized { .. } => {
                // Session metadata event — no projection state change needed
            }
            EventPayload::TaskDecomposed { .. } => {
                // Planner decomposed a task into sub-tasks — handled via individual
                // AgentTaskCreated/AgentTaskStarted events emitted for each sub-task.
            }
            EventPayload::TaskBlocked {
                task_id,
                blocking_task_id: _,
                reason,
            } => {
                if let Some(t) = self.task_graph.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.state = TaskState::Blocked;
                    t.error = Some(reason.clone());
                }
            }
            EventPayload::AgentSpawned {
                agent_id,
                role: _,
                task_id,
            } => {
                if let Some(t) = self.task_graph.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.assigned_agent_id = Some(agent_id.clone());
                }
            }
            EventPayload::AgentIdle { .. } => {
                // Agent finished work — no projection state change needed
            }
            EventPayload::TaskRetried { task_id, attempt } => {
                if let Some(t) = self.task_graph.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.state = TaskState::Pending;
                    t.retry_count = *attempt;
                    t.error = None;
                    t.failure_reason = None;
                }
            }
            // Events not relevant to session projection
            EventPayload::WorkspaceOpened { .. }
            | EventPayload::ContextAssembled { .. }
            | EventPayload::ModelRequestStarted { .. }
            | EventPayload::ModelToolCallRequested { .. }
            | EventPayload::PermissionRequested { .. }
            | EventPayload::PermissionGranted { .. }
            | EventPayload::PermissionDenied { .. }
            | EventPayload::ToolInvocationStarted { .. }
            | EventPayload::ToolInvocationCompleted { .. }
            | EventPayload::ToolInvocationFailed { .. }
            | EventPayload::FilePatchProposed { .. }
            | EventPayload::FilePatchApplied { .. }
            | EventPayload::MemoryProposed { .. }
            | EventPayload::MemoryAccepted { .. }
            | EventPayload::MemoryRejected { .. }
            | EventPayload::ReviewerFindingAdded { .. }
            | EventPayload::McpServerStarting { .. }
            | EventPayload::McpServerReady { .. }
            | EventPayload::McpServerStopped { .. }
            | EventPayload::McpServerFailed { .. }
            | EventPayload::McpToolCallStarted { .. }
            | EventPayload::McpToolCallCompleted { .. }
            | EventPayload::McpTrustGranted { .. }
            | EventPayload::McpTrustRevoked { .. }
            | EventPayload::CatalogRefreshed { .. }
            | EventPayload::CatalogEntryInstalling { .. }
            | EventPayload::CatalogEntryInstalled { .. }
            | EventPayload::CatalogEntryUninstalled { .. }
            | EventPayload::CatalogRuntimeMissing { .. }
            | EventPayload::CatalogSourceAdded { .. }
            | EventPayload::CatalogSourceFailed { .. }
            | EventPayload::ContextCompactionStarted { .. }
            | EventPayload::ContextCompactionCompleted { .. }
            | EventPayload::ContextCompactionFailed { .. }
            | EventPayload::CompactionSummary { .. } => {}
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

    #[test]
    fn compaction_status_serializes_with_internal_tag() {
        let s = CompactionStatus::Idle;
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["type"], "Idle");

        let s = CompactionStatus::Running;
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["type"], "Running");

        let s = CompactionStatus::Failed {
            error: "llm timeout".into(),
        };
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["type"], "Failed");
        assert_eq!(json["error"], "llm timeout");

        let back: CompactionStatus = serde_json::from_value(json).unwrap();
        assert!(matches!(back, CompactionStatus::Failed { .. }));
    }

    #[test]
    fn compaction_status_default_is_idle() {
        let s = CompactionStatus::default();
        assert!(matches!(s, CompactionStatus::Idle));
    }
}

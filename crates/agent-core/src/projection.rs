use crate::events::{DomainEvent, EventPayload};
use crate::facade::TaskGraphSnapshot;
use crate::{TaskFailureReason, TaskSnapshot, TaskState};
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
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub context_window: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub output_limit: u64,
    /// Snake-case `LimitSource` discriminant: "user_config" | "builtin_registry" | "runtime_probe" | "fallback".
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SessionProjection {
    pub messages: Vec<ProjectedMessage>,
    pub task_titles: Vec<String>,
    pub task_graph: TaskGraphSnapshot,
    pub token_stream: String,
    pub cancelled: bool,
    #[serde(default)]
    pub last_context_usage: Option<crate::context_types::ContextUsage>,
    #[serde(default)]
    pub model_limits: Option<ProjectedModelLimits>,
    #[serde(default)]
    pub compaction: CompactionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectedMessage {
    pub role: ProjectedRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
            EventPayload::TaskCancelled { task_id } => {
                if let Some(t) = self.task_graph.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.state = TaskState::Cancelled;
                    t.error = None;
                    t.failure_reason = Some(TaskFailureReason::Cancelled);
                }
            }
            EventPayload::ContextAssembled { usage } => {
                self.last_context_usage = Some(usage.clone());
            }
            EventPayload::ContextCompactionStarted { .. } => {
                self.compaction = CompactionStatus::Running;
            }
            EventPayload::ContextCompactionCompleted { .. } => {
                self.compaction = CompactionStatus::Idle;
            }
            EventPayload::ContextCompactionFailed { error, .. } => {
                self.compaction = CompactionStatus::Failed {
                    error: error.clone(),
                };
            }
            EventPayload::ModelProfileSwitched {
                context_window,
                output_limit,
                limit_source,
                ..
            } => {
                self.model_limits = Some(ProjectedModelLimits {
                    context_window: *context_window,
                    output_limit: *output_limit,
                    source: limit_source.clone(),
                });
            }
            // Events not relevant to session projection
            EventPayload::WorkspaceOpened { .. }
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
            | EventPayload::SkillDiscovered { .. }
            | EventPayload::SkillValidationFailed { .. }
            | EventPayload::SkillActivated { .. }
            | EventPayload::SkillDeactivated { .. }
            | EventPayload::SkillSuggested { .. }
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
            | EventPayload::CatalogSourceResultsArrived { .. }
            | EventPayload::CompactionSummary { .. }
            | EventPayload::ContextCompactionSkipped { .. } => {}
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
            last_context_usage: None,
            model_limits: None,
            compaction: CompactionStatus::default(),
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

    #[test]
    fn projects_context_assembled_into_last_context_usage() {
        use crate::context_types::{ContextSource, ContextUsage};
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();
        let usage = ContextUsage {
            total_tokens: 12_000,
            budget_tokens: 180_000,
            context_window: 200_000,
            output_reservation: 20_000,
            by_source: vec![
                (ContextSource::System, 2_000),
                (ContextSource::History, 10_000),
            ],
            estimator: "cl100k_base".to_string(),
            corrected_by_real_usage: false,
        };

        let event = DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::ContextAssembled {
                usage: usage.clone(),
            },
        );

        let projection = SessionProjection::from_events(&[event]);

        let cached = projection.last_context_usage.expect("usage should be set");
        assert_eq!(cached.total_tokens, 12_000);
        assert_eq!(cached.budget_tokens, 180_000);
        assert_eq!(cached.by_source.len(), 2);
    }

    #[test]
    fn projects_compaction_lifecycle_into_compaction_status() {
        use crate::events::CompactionReason;
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();

        let started = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::ContextCompactionStarted {
                reason: CompactionReason::UserRequested,
                before_tokens: 180_000u64,
                candidate_event_count: 42usize,
            },
        );
        let completed = DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::ContextCompactionCompleted {
                summary_id: "sum_1".into(),
                after_tokens: 30_000u64,
                fallback_used: false,
            },
        );

        let only_started = SessionProjection::from_events(std::slice::from_ref(&started));
        assert!(matches!(only_started.compaction, CompactionStatus::Running));

        let started_then_done = SessionProjection::from_events(&[started, completed]);
        assert!(matches!(
            started_then_done.compaction,
            CompactionStatus::Idle
        ));
    }

    #[test]
    fn projects_compaction_failed_into_failed_status() {
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();
        let failed = DomainEvent::new(
            workspace_id,
            session_id,
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::ContextCompactionFailed {
                error: "model timeout".into(),
                fallback_used: true,
            },
        );

        let projection = SessionProjection::from_events(&[failed]);
        match projection.compaction {
            CompactionStatus::Failed { error } => assert_eq!(error, "model timeout"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }
}

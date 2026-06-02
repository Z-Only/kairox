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
                self.cancelled = false;
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
            | EventPayload::ContextCompactionSkipped { .. }
            | EventPayload::MonitorStarted { .. }
            | EventPayload::MonitorEvent { .. }
            | EventPayload::MonitorStopped { .. }
            | EventPayload::MonitorFailed { .. }
            | EventPayload::LspServerStarting { .. }
            | EventPayload::LspServerReady { .. }
            | EventPayload::LspServerStopped { .. }
            | EventPayload::LspServerFailed { .. }
            | EventPayload::DapSessionStarted { .. }
            | EventPayload::DapSessionStopped { .. }
            | EventPayload::DapBreakpointHit { .. }
            | EventPayload::TrajectoryStarted { .. }
            | EventPayload::TrajectoryStepRecorded { .. }
            | EventPayload::TrajectoryCompleted { .. } => {}
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
#[path = "projection_tests.rs"]
mod tests;

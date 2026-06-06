use crate::autonomous::checkpoint_writer::CheckpointWriter;
use crate::autonomous::orientation::OrientationPromptBuilder;
use crate::event_emitter::append_and_broadcast;
use agent_core::autonomous::{
    AutonomousConfig, AutonomousTaskGoal, AutonomousTaskState, SessionEndReason,
};
use agent_core::{
    AgentId, AutonomousTaskId, DomainEvent, EventPayload, PrivacyClassification,
    SendMessageRequest, SessionId, WorkspaceId,
};
use agent_store::{AutonomousCheckpointRow, AutonomousTaskRow, AutonomousTaskStore, EventStore};
use std::sync::Arc;

pub struct AutonomousController<S: EventStore + 'static> {
    store: Arc<S>,
    autonomous_store: Arc<dyn AutonomousTaskStore>,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
}

impl<S: EventStore + 'static> AutonomousController<S> {
    pub fn new(
        store: Arc<S>,
        autonomous_store: Arc<dyn AutonomousTaskStore>,
        event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    ) -> Self {
        Self {
            store,
            autonomous_store,
            event_tx,
        }
    }

    pub async fn start_autonomous_task(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &SessionId,
        goal: AutonomousTaskGoal,
        config: AutonomousConfig,
    ) -> agent_core::Result<AutonomousTaskId> {
        let task_id = AutonomousTaskId::new();
        let now = chrono::Utc::now().to_rfc3339();

        let row = AutonomousTaskRow {
            autonomous_task_id: task_id.to_string(),
            workspace_id: workspace_id.to_string(),
            goal_json: serde_json::to_string(&goal)
                .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?,
            config_json: serde_json::to_string(&config)
                .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?,
            state: AutonomousTaskState::Active.to_string(),
            current_session_id: Some(session_id.to_string()),
            session_count: 1,
            created_at: now.clone(),
            updated_at: now,
        };

        self.autonomous_store
            .create_autonomous_task(&row)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        self.autonomous_store
            .insert_session_chain_entry(&task_id.to_string(), session_id.as_str(), 0)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AutonomousTaskCreated {
                autonomous_task_id: task_id.clone(),
                goal: goal.description.clone(),
                acceptance_criteria: goal.acceptance_criteria.clone(),
                max_sessions: config.max_sessions,
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AutonomousTaskSessionStarted {
                autonomous_task_id: task_id.clone(),
                session_id: session_id.clone(),
                session_index: 0,
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

        Ok(task_id)
    }

    pub async fn on_session_ended(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &SessionId,
        task_id: &AutonomousTaskId,
    ) -> agent_core::Result<Option<ContinuationAction>> {
        let task_row = self
            .autonomous_store
            .get_autonomous_task(task_id.as_str())
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!("autonomous task not found: {task_id}"))
            })?;

        let goal: AutonomousTaskGoal = serde_json::from_str(&task_row.goal_json)
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        let config: AutonomousConfig = serde_json::from_str(&task_row.config_json)
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        let events = self
            .store
            .load_session(session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        let session_index = task_row.session_count as u32 - 1;
        let checkpoint = CheckpointWriter::build_checkpoint(
            &events,
            &goal,
            session_id,
            session_index,
            None,
            vec![],
        );
        let end_reason = CheckpointWriter::detect_end_reason(&events);

        let checkpoint_json = serde_json::to_string(&checkpoint)
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        self.autonomous_store
            .insert_checkpoint(&AutonomousCheckpointRow {
                checkpoint_id: checkpoint.checkpoint_id.clone(),
                autonomous_task_id: task_id.to_string(),
                session_id: session_id.to_string(),
                session_index: session_index as i64,
                checkpoint_json: checkpoint_json.clone(),
                end_reason: end_reason.to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AutonomousTaskCheckpointed {
                autonomous_task_id: task_id.clone(),
                session_id: session_id.clone(),
                session_index,
                checkpoint_json,
                end_reason: end_reason.to_string(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

        match end_reason {
            SessionEndReason::TaskCompleted => {
                self.autonomous_store
                    .update_autonomous_task_state(
                        task_id.as_str(),
                        &AutonomousTaskState::Completed.to_string(),
                        None,
                    )
                    .await
                    .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

                let event = DomainEvent::new(
                    workspace_id.clone(),
                    session_id.clone(),
                    AgentId::system(),
                    PrivacyClassification::MinimalTrace,
                    EventPayload::AutonomousTaskCompleted {
                        autonomous_task_id: task_id.clone(),
                        total_sessions: task_row.session_count as u32,
                    },
                );
                append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
                Ok(None)
            }
            SessionEndReason::TaskFailed => {
                self.autonomous_store
                    .update_autonomous_task_state(
                        task_id.as_str(),
                        &AutonomousTaskState::Failed.to_string(),
                        None,
                    )
                    .await
                    .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

                let event = DomainEvent::new(
                    workspace_id.clone(),
                    session_id.clone(),
                    AgentId::system(),
                    PrivacyClassification::MinimalTrace,
                    EventPayload::AutonomousTaskFailed {
                        autonomous_task_id: task_id.clone(),
                        reason: "task failed".into(),
                    },
                );
                append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
                Ok(None)
            }
            SessionEndReason::UserPaused => {
                self.autonomous_store
                    .update_autonomous_task_state(
                        task_id.as_str(),
                        &AutonomousTaskState::Paused.to_string(),
                        None,
                    )
                    .await
                    .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
                Ok(None)
            }
            SessionEndReason::ContextLimitReached | SessionEndReason::MaxIterationsReached => {
                if !config.auto_continue {
                    self.autonomous_store
                        .update_autonomous_task_state(
                            task_id.as_str(),
                            &AutonomousTaskState::Paused.to_string(),
                            None,
                        )
                        .await
                        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
                    return Ok(None);
                }

                let next_index = task_row.session_count as u32;
                if next_index >= config.max_sessions {
                    self.autonomous_store
                        .update_autonomous_task_state(
                            task_id.as_str(),
                            &AutonomousTaskState::Failed.to_string(),
                            None,
                        )
                        .await
                        .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

                    let event = DomainEvent::new(
                        workspace_id.clone(),
                        session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::MinimalTrace,
                        EventPayload::AutonomousTaskFailed {
                            autonomous_task_id: task_id.clone(),
                            reason: format!(
                                "max sessions reached ({}/{})",
                                next_index, config.max_sessions
                            ),
                        },
                    );
                    append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
                    return Ok(None);
                }

                let orientation = OrientationPromptBuilder::build(
                    &goal,
                    &checkpoint,
                    next_index,
                    config.max_sessions,
                );

                Ok(Some(ContinuationAction {
                    task_id: task_id.clone(),
                    workspace_id: workspace_id.clone(),
                    session_index: next_index,
                    orientation_prompt: orientation,
                    goal,
                    config,
                }))
            }
        }
    }

    pub async fn register_continuation_session(
        &self,
        task_id: &AutonomousTaskId,
        workspace_id: &WorkspaceId,
        new_session_id: &SessionId,
        session_index: u32,
    ) -> agent_core::Result<()> {
        self.autonomous_store
            .insert_session_chain_entry(task_id.as_str(), new_session_id.as_str(), session_index)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        self.autonomous_store
            .increment_session_count(task_id.as_str())
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        self.autonomous_store
            .update_autonomous_task_state(
                task_id.as_str(),
                &AutonomousTaskState::Active.to_string(),
                Some(new_session_id.as_str()),
            )
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        let event = DomainEvent::new(
            workspace_id.clone(),
            new_session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AutonomousTaskSessionStarted {
                autonomous_task_id: task_id.clone(),
                session_id: new_session_id.clone(),
                session_index,
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

        Ok(())
    }

    pub async fn cancel_autonomous_task(
        &self,
        workspace_id: &WorkspaceId,
        session_id: &SessionId,
        task_id: &AutonomousTaskId,
    ) -> agent_core::Result<()> {
        self.autonomous_store
            .update_autonomous_task_state(
                task_id.as_str(),
                &AutonomousTaskState::Cancelled.to_string(),
                None,
            )
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        let event = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::AutonomousTaskCancelled {
                autonomous_task_id: task_id.clone(),
            },
        );
        append_and_broadcast(&*self.store, &self.event_tx, &event).await?;
        Ok(())
    }

    pub fn build_continuation_message(action: &ContinuationAction) -> SendMessageRequest {
        SendMessageRequest {
            workspace_id: action.workspace_id.clone(),
            session_id: SessionId::new(),
            content: action.orientation_prompt.clone(),
            display_content: Some("[Autonomous continuation]".into()),
            attachments: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContinuationAction {
    pub task_id: AutonomousTaskId,
    pub workspace_id: WorkspaceId,
    pub session_index: u32,
    pub orientation_prompt: String,
    pub goal: AutonomousTaskGoal,
    pub config: AutonomousConfig,
}

#[cfg(test)]
#[path = "controller_tests.rs"]
mod tests;

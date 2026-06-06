use agent_core::autonomous::AutonomousTaskState;
use agent_core::{
    AutonomousFacade, AutonomousTaskId, AutonomousTaskView, CheckpointView, SessionId, WorkspaceId,
};
use agent_store::EventStore;
use async_trait::async_trait;

use crate::facade_runtime::LocalRuntime;

#[async_trait]
impl<S, M> AutonomousFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn list_autonomous_tasks(
        &self,
        workspace_id: WorkspaceId,
    ) -> agent_core::Result<Vec<AutonomousTaskView>> {
        let Some(store) = &self.autonomous_store else {
            return Ok(vec![]);
        };
        let rows = store
            .list_active_autonomous_tasks(workspace_id.as_str())
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        Ok(rows.into_iter().filter_map(|r| row_to_view(&r)).collect())
    }

    async fn get_autonomous_task(
        &self,
        task_id: AutonomousTaskId,
    ) -> agent_core::Result<Option<AutonomousTaskView>> {
        let Some(store) = &self.autonomous_store else {
            return Ok(None);
        };
        let row = store
            .get_autonomous_task(task_id.as_str())
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        Ok(row.as_ref().and_then(row_to_view))
    }

    async fn get_autonomous_checkpoints(
        &self,
        task_id: AutonomousTaskId,
    ) -> agent_core::Result<Vec<CheckpointView>> {
        let Some(store) = &self.autonomous_store else {
            return Ok(vec![]);
        };
        let rows = store
            .list_checkpoints(task_id.as_str())
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        Ok(rows
            .into_iter()
            .filter_map(checkpoint_row_to_view)
            .collect())
    }

    async fn pause_autonomous_task(&self, task_id: AutonomousTaskId) -> agent_core::Result<()> {
        let Some(store) = &self.autonomous_store else {
            return Err(agent_core::CoreError::InvalidState(
                "autonomous store not available".into(),
            ));
        };
        store
            .update_autonomous_task_state(
                task_id.as_str(),
                &AutonomousTaskState::Paused.to_string(),
                None,
            )
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))
    }

    async fn resume_autonomous_task(&self, task_id: AutonomousTaskId) -> agent_core::Result<()> {
        let Some(store) = &self.autonomous_store else {
            return Err(agent_core::CoreError::InvalidState(
                "autonomous store not available".into(),
            ));
        };
        store
            .update_autonomous_task_state(
                task_id.as_str(),
                &AutonomousTaskState::Active.to_string(),
                None,
            )
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))
    }

    async fn cancel_autonomous_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: AutonomousTaskId,
    ) -> agent_core::Result<()> {
        if let Some(controller) = &self.autonomous_controller {
            controller
                .cancel_autonomous_task(&workspace_id, &session_id, &task_id)
                .await
        } else {
            Err(agent_core::CoreError::InvalidState(
                "autonomous controller not available".into(),
            ))
        }
    }
}

fn row_to_view(row: &agent_store::AutonomousTaskRow) -> Option<AutonomousTaskView> {
    let goal: agent_core::AutonomousTaskGoal = serde_json::from_str(&row.goal_json).ok()?;
    let config: agent_core::AutonomousConfig = serde_json::from_str(&row.config_json).ok()?;
    Some(AutonomousTaskView {
        autonomous_task_id: AutonomousTaskId::from_string(row.autonomous_task_id.clone()),
        workspace_id: WorkspaceId::from(row.workspace_id.clone()),
        goal: goal.description,
        state: row.state.clone(),
        current_session_id: row.current_session_id.clone(),
        session_count: row.session_count as u32,
        max_sessions: config.max_sessions,
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
    })
}

fn checkpoint_row_to_view(row: agent_store::AutonomousCheckpointRow) -> Option<CheckpointView> {
    let checkpoint: agent_core::Checkpoint = serde_json::from_str(&row.checkpoint_json).ok()?;
    Some(CheckpointView {
        checkpoint_id: row.checkpoint_id,
        session_id: SessionId::from_string(row.session_id),
        session_index: row.session_index as u32,
        completed_items: checkpoint.completed_items,
        remaining_items: checkpoint.remaining_items,
        git_sha: checkpoint.git_sha,
        end_reason: row.end_reason,
        created_at: row.created_at,
    })
}

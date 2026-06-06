use crate::{AutonomousTaskId, SessionId, WorkspaceId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AutonomousTaskView {
    pub autonomous_task_id: AutonomousTaskId,
    pub workspace_id: WorkspaceId,
    pub goal: String,
    pub state: String,
    pub current_session_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub session_count: u32,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub max_sessions: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CheckpointView {
    pub checkpoint_id: String,
    pub session_id: SessionId,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub session_index: u32,
    pub completed_items: Vec<String>,
    pub remaining_items: Vec<String>,
    pub git_sha: Option<String>,
    pub end_reason: String,
    pub created_at: String,
}

#[async_trait]
pub trait AutonomousFacade: Send + Sync {
    async fn list_autonomous_tasks(
        &self,
        workspace_id: WorkspaceId,
    ) -> crate::Result<Vec<AutonomousTaskView>> {
        let _ = workspace_id;
        Ok(vec![])
    }

    async fn get_autonomous_task(
        &self,
        task_id: AutonomousTaskId,
    ) -> crate::Result<Option<AutonomousTaskView>> {
        let _ = task_id;
        Ok(None)
    }

    async fn get_autonomous_checkpoints(
        &self,
        task_id: AutonomousTaskId,
    ) -> crate::Result<Vec<CheckpointView>> {
        let _ = task_id;
        Ok(vec![])
    }

    async fn pause_autonomous_task(&self, task_id: AutonomousTaskId) -> crate::Result<()> {
        let _ = task_id;
        Ok(())
    }

    async fn resume_autonomous_task(&self, task_id: AutonomousTaskId) -> crate::Result<()> {
        let _ = task_id;
        Ok(())
    }

    async fn cancel_autonomous_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: AutonomousTaskId,
    ) -> crate::Result<()> {
        let _ = (workspace_id, session_id, task_id);
        Ok(())
    }
}

use agent_core::trajectory::{TrajectoryId, TrajectoryMeta, TrajectoryOutcome, TrajectoryStep};
use async_trait::async_trait;

mod sqlite;

pub use sqlite::SqliteTrajectoryStore;

/// Persistence layer for trajectory data (step-by-step agent action records).
#[async_trait]
pub trait TrajectoryStore: Send + Sync {
    /// Start a new trajectory for a task.
    async fn start_trajectory(
        &self,
        trajectory_id: &TrajectoryId,
        task_id: &str,
        session_id: &str,
    ) -> crate::Result<()>;

    /// Record a step in an existing trajectory.
    async fn record_step(
        &self,
        trajectory_id: &TrajectoryId,
        step: &TrajectoryStep,
    ) -> crate::Result<()>;

    /// Mark a trajectory as completed with an outcome.
    async fn complete_trajectory(
        &self,
        trajectory_id: &TrajectoryId,
        outcome: TrajectoryOutcome,
    ) -> crate::Result<()>;

    /// Load all steps for a trajectory in order.
    async fn load_steps(&self, trajectory_id: &TrajectoryId) -> crate::Result<Vec<TrajectoryStep>>;

    /// Load trajectory metadata.
    async fn get_meta(&self, trajectory_id: &TrajectoryId)
        -> crate::Result<Option<TrajectoryMeta>>;

    /// List trajectories for a session.
    async fn list_by_session(&self, session_id: &str) -> crate::Result<Vec<TrajectoryMeta>>;

    /// Export a trajectory as JSON (for replay/eval).
    async fn export_json(&self, trajectory_id: &TrajectoryId) -> crate::Result<serde_json::Value>;
}

#[cfg(test)]
mod tests;

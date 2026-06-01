use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique identifier for a trajectory (sequence of agent steps for a task).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TrajectoryId(pub String);

impl TrajectoryId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for TrajectoryId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TrajectoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A single step in an agent's trajectory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TrajectoryStep {
    /// Sequential step number within the trajectory (0-indexed).
    pub step_index: u32,
    /// The action taken (tool_id or "message").
    pub action: String,
    /// Tool arguments or message content (summarized).
    pub action_input: serde_json::Value,
    /// The observation/result of the action.
    pub observation: String,
    /// Optional screenshot identifier if a visual was captured.
    pub screenshot_id: Option<String>,
    /// When this step occurred.
    pub timestamp: DateTime<Utc>,
    /// Duration of the action in milliseconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub duration_ms: u64,
}

/// Summary metadata for a completed trajectory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TrajectoryMeta {
    pub trajectory_id: TrajectoryId,
    pub task_id: String,
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub step_count: u32,
    pub outcome: TrajectoryOutcome,
}

/// How a trajectory ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum TrajectoryOutcome {
    Success,
    Failed,
    Cancelled,
    InProgress,
}

#[cfg(test)]
#[path = "trajectory_tests.rs"]
mod tests;

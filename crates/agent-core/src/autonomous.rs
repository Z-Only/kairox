use crate::SessionId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AutonomousTaskState {
    Active,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for AutonomousTaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for AutonomousTaskState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(format!("unknown AutonomousTaskState: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AutonomousTaskGoal {
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub verification_commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AutonomousConfig {
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub max_sessions: u32,
    pub auto_continue: bool,
    pub verification_required: bool,
    pub git_checkpoint: bool,
}

impl Default for AutonomousConfig {
    fn default() -> Self {
        Self {
            max_sessions: 10,
            auto_continue: true,
            verification_required: true,
            git_checkpoint: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct VerificationResult {
    pub criterion: String,
    pub passed: bool,
    pub output_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Checkpoint {
    pub checkpoint_id: String,
    pub session_id: SessionId,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub session_index: u32,
    pub git_sha: Option<String>,
    pub completed_items: Vec<String>,
    pub remaining_items: Vec<String>,
    pub verification_results: Vec<VerificationResult>,
    pub notes: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SessionEndReason {
    ContextLimitReached,
    MaxIterationsReached,
    UserPaused,
    TaskCompleted,
    TaskFailed,
}

impl std::fmt::Display for SessionEndReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContextLimitReached => write!(f, "context_limit_reached"),
            Self::MaxIterationsReached => write!(f, "max_iterations_reached"),
            Self::UserPaused => write!(f, "user_paused"),
            Self::TaskCompleted => write!(f, "task_completed"),
            Self::TaskFailed => write!(f, "task_failed"),
        }
    }
}

impl std::str::FromStr for SessionEndReason {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "context_limit_reached" => Ok(Self::ContextLimitReached),
            "max_iterations_reached" => Ok(Self::MaxIterationsReached),
            "user_paused" => Ok(Self::UserPaused),
            "task_completed" => Ok(Self::TaskCompleted),
            "task_failed" => Ok(Self::TaskFailed),
            other => Err(format!("unknown SessionEndReason: {other}")),
        }
    }
}

#[cfg(test)]
#[path = "autonomous_tests.rs"]
mod tests;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "PascalCase")]
pub enum AgentRole {
    Planner,
    Worker,
    Reviewer,
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Planner => write!(f, "Planner"),
            Self::Worker => write!(f, "Worker"),
            Self::Reviewer => write!(f, "Reviewer"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "PascalCase")]
pub enum TaskState {
    Pending,
    Ready,
    Running,
    Blocked,
    Completed,
    Failed,
    Skipped,
    Cancelled,
}

impl TaskState {
    /// Returns true if the task is in a terminal state (no further transitions).
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Skipped | Self::Cancelled
        )
    }
}

/// Reason a task failed, used for diagnostics and UI display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum TaskFailureReason {
    ModelError {
        #[cfg_attr(feature = "specta", specta(type = u32))]
        retries: usize,
    },
    ToolExhausted {
        tool_id: String,
        #[cfg_attr(feature = "specta", specta(type = u32))]
        attempts: usize,
        last_error: String,
    },
    PermissionDenied {
        tool_id: String,
    },
    Cancelled,
    MaxIterations,
}

/// Policy for how a task failure affects its dependents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "PascalCase")]
pub enum FailurePolicy {
    /// Block all transitive dependents (default).
    #[default]
    BlockDependents,
    /// Dependents receive "parent failed" context and may proceed.
    AllowOrphans,
    /// Cancel the entire DAG.
    FailFast,
}

/// Backoff strategy for retries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum BackoffStrategy {
    Fixed { delay_ms: u64 },
    ExponentialJitter { base_ms: u64, max_ms: u64 },
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        Self::ExponentialJitter {
            base_ms: 1000,
            max_ms: 30_000,
        }
    }
}

/// Configuration for retry behaviour in the DAG executor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RetryConfig {
    /// Maximum retries for model errors (default: 3).
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub max_model_retries: usize,
    /// Maximum retries for tool call exhaustion (default: 2).
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub max_tool_retries: usize,
    /// Backoff strategy between retries (default: ExponentialJitter).
    pub backoff: BackoffStrategy,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_model_retries: 3,
            max_tool_retries: 2,
            backoff: BackoffStrategy::default(),
        }
    }
}

#[cfg(test)]
#[path = "task_types_tests.rs"]
mod tests;

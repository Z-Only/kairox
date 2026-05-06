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
        retries: usize,
    },
    ToolExhausted {
        tool_id: String,
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
    pub max_model_retries: usize,
    /// Maximum retries for tool call exhaustion (default: 2).
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
mod tests {
    use super::*;

    #[test]
    fn task_state_is_terminal() {
        assert!(!TaskState::Pending.is_terminal());
        assert!(!TaskState::Ready.is_terminal());
        assert!(!TaskState::Running.is_terminal());
        assert!(!TaskState::Blocked.is_terminal());
        assert!(TaskState::Completed.is_terminal());
        assert!(TaskState::Failed.is_terminal());
        assert!(TaskState::Skipped.is_terminal());
        assert!(TaskState::Cancelled.is_terminal());
    }

    #[test]
    fn default_failure_policy_is_block_dependents() {
        assert_eq!(FailurePolicy::default(), FailurePolicy::BlockDependents);
    }

    #[test]
    fn default_retry_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_model_retries, 3);
        assert_eq!(config.max_tool_retries, 2);
        assert!(matches!(
            config.backoff,
            BackoffStrategy::ExponentialJitter {
                base_ms: 1000,
                max_ms: 30_000
            }
        ));
    }

    #[test]
    fn failure_reason_serialization_roundtrip() {
        let reason = TaskFailureReason::ToolExhausted {
            tool_id: "fs.read".into(),
            attempts: 3,
            last_error: "permission denied".into(),
        };
        let json = serde_json::to_string(&reason).unwrap();
        let deserialized: TaskFailureReason = serde_json::from_str(&json).unwrap();
        assert_eq!(reason, deserialized);
    }
}

use agent_core::{FailurePolicy, RetryConfig};

/// Configuration for the DAG executor.
#[derive(Debug, Clone)]
pub struct DagConfig {
    /// Maximum number of tasks that can execute concurrently.
    pub max_concurrency: usize,
    /// Failure policy for task failures.
    pub failure_policy: FailurePolicy,
    /// Retry configuration for model and tool errors.
    pub retry_config: RetryConfig,
}

impl Default for DagConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 3,
            failure_policy: FailurePolicy::BlockDependents,
            retry_config: RetryConfig::default(),
        }
    }
}

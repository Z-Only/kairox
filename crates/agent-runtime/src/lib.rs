pub mod agent_loop;
pub mod agents;
pub(crate) mod catalog_sink;
pub mod compaction;
pub mod context_budget;
pub mod dag_executor;
pub mod event_emitter;
pub mod facade_runtime;
pub(crate) mod marketplace_toml;
pub mod mcp_manager;
pub mod memory_handler;
pub mod permission;
pub mod project;
pub mod session;
pub mod skill_package;
pub mod skills;
pub mod task_graph;

pub mod test_support;

pub use agent_core::{
    AgentRole, BackoffStrategy, FailurePolicy, RetryConfig, TaskFailureReason, TaskState,
};
pub use agents::planner_agent::PlannerStrategy;
pub use agents::reviewer_agent::ReviewerStrategy;
pub use agents::worker_agent::WorkerStrategy;
pub use agents::{
    AgentDecision, AgentStrategy, PlannerAgent, ReviewerAgent, ReviewerFinding, StepContext,
    StepOutcome, SubTaskDef, ToolResultAction, WorkerAgent,
};
pub use dag_executor::{AgentStatus, DagConfig, DagExecutor, ExecutionResult};
pub use facade_runtime::{ExecutionMode, LocalRuntime};
pub use mcp_manager::McpServerManager;
pub use task_graph::{AgentTask, TaskGraph, TaskStateCounts};

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("unknown task: {0}")]
    UnknownTask(String),
    #[error("agent loop exceeded maximum iterations")]
    MaxIterationsExceeded,
    #[error("permission required: {0}")]
    PermissionRequired(String),
    #[error("DAG execution failed: {0}")]
    DagExecutionFailed(String),
    #[error("task not found: {0}")]
    TaskNotFound(String),
    #[error("task cannot be retried: {0}")]
    TaskCannotRetry(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, RuntimeError>;

pub mod agents;
pub mod facade_runtime;
pub mod task_graph;

pub use agents::{PlannerAgent, ReviewerAgent, ReviewerFinding, WorkerAgent};
pub use facade_runtime::LocalRuntime;
pub use task_graph::{AgentRole, AgentTask, TaskGraph, TaskState};

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("unknown task: {0}")]
    UnknownTask(String),
    #[error("agent loop exceeded maximum iterations")]
    MaxIterationsExceeded,
    #[error("permission required: {0}")]
    PermissionRequired(String),
}

pub type Result<T> = std::result::Result<T, RuntimeError>;

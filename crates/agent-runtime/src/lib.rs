pub mod agent_loop;
pub mod agents;
pub mod dag_executor;
pub mod event_emitter;
pub mod facade_runtime;
pub mod mcp_manager;
pub mod memory_handler;
pub mod permission;
pub mod session;
pub mod task_graph;

pub use agent_core::{AgentRole, TaskState};
pub use agents::{PlannerAgent, ReviewerAgent, ReviewerFinding, WorkerAgent};
pub use dag_executor::{DagConfig, DagExecutor};
pub use facade_runtime::LocalRuntime;
pub use mcp_manager::McpServerManager;
pub use task_graph::{AgentTask, TaskGraph};

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

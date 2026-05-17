use crate::task_graph::TaskGraph;
use agent_core::{AgentRole, TaskId};

/// Result of a DAG execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// The final task graph after execution.
    pub graph: TaskGraph,
    /// Total number of tasks in the graph.
    pub total_tasks: usize,
    /// Number of completed tasks.
    pub completed: usize,
    /// Number of failed tasks.
    pub failed: usize,
    /// Number of skipped tasks.
    pub skipped: usize,
}

/// Status information about a running or completed agent.
#[derive(Debug, Clone)]
pub struct AgentStatus {
    pub agent_id: String,
    pub role: AgentRole,
    pub task_id: Option<TaskId>,
    pub status: String,
}

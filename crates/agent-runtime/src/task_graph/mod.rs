//! Task graph used by the agent runtime to schedule and track work units.
//!
//! Split into focused submodules:
//! - [`build`]: task creation and acyclic validation
//! - [`state`]: per-task state transitions (`mark_running`, `mark_failed`, ...)
//! - [`query`]: read-only queries and traversal (`ready_tasks`, `find_*`,
//!   `snapshot`, `state_counts`)

use agent_core::{AgentId, AgentRole, TaskFailureReason, TaskId, TaskState};
use std::collections::BTreeMap;

mod build;
mod query;
mod state;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTask {
    pub id: TaskId,
    pub title: String,
    pub description: String,
    pub role: AgentRole,
    pub state: TaskState,
    pub dependencies: Vec<TaskId>,
    pub error: Option<String>,
    pub retry_count: usize,
    pub max_retries: usize,
    pub assigned_agent_id: Option<AgentId>,
    pub failure_reason: Option<TaskFailureReason>,
}

#[derive(Debug, Default, Clone)]
pub struct TaskGraph {
    tasks: BTreeMap<String, AgentTask>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskStateCounts {
    pub pending: usize,
    pub ready: usize,
    pub running: usize,
    pub blocked: usize,
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cancelled: usize,
}

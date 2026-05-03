use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AgentRole {
    Planner,
    Worker,
    Reviewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TaskState {
    Pending,
    Running,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

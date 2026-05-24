//! Task construction: creating tasks and validating that the graph stays acyclic.

use agent_core::{AgentId, AgentRole, TaskId, TaskState};

use super::{AgentTask, TaskGraph};

impl TaskGraph {
    /// Add a task with only title, role, and dependencies.
    /// Uses defaults for retry/agent fields.
    pub fn add_task(
        &mut self,
        title: impl Into<String>,
        role: AgentRole,
        dependencies: Vec<TaskId>,
    ) -> TaskId {
        let id = TaskId::new();
        let task = AgentTask {
            id: id.clone(),
            title: title.into(),
            description: String::new(),
            role,
            state: TaskState::Pending,
            dependencies,
            error: None,
            retry_count: 0,
            max_retries: 2,
            assigned_agent_id: None,
            failure_reason: None,
        };
        self.tasks.insert(id.to_string(), task);
        id
    }

    /// Add a task with full configuration including retry and agent assignment.
    pub fn add_task_with_config(
        &mut self,
        title: impl Into<String>,
        role: AgentRole,
        dependencies: Vec<TaskId>,
        max_retries: usize,
        assigned_agent_id: Option<AgentId>,
    ) -> TaskId {
        let id = TaskId::new();
        let task = AgentTask {
            id: id.clone(),
            title: title.into(),
            description: String::new(),
            role,
            state: TaskState::Pending,
            dependencies,
            error: None,
            retry_count: 0,
            max_retries,
            assigned_agent_id,
            failure_reason: None,
        };
        self.tasks.insert(id.to_string(), task);
        id
    }

    /// Validate that adding a task with given dependencies would not create a cycle.
    /// Returns Ok(()) if the graph would remain acyclic, Err otherwise.
    pub fn validate_acyclic(&self, new_dependencies: &[TaskId]) -> crate::Result<()> {
        // Check that all dependency IDs exist in the graph
        for dep in new_dependencies {
            if !self.tasks.contains_key(&dep.to_string()) {
                return Err(crate::RuntimeError::UnknownTask(dep.to_string()));
            }
        }
        // For a DAG where we only add tasks with references to existing tasks,
        // cycles are impossible because new tasks can only depend on already-added tasks.
        // This validation is a placeholder for future extensions where tasks could
        // be added with forward references.
        Ok(())
    }
}

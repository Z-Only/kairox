//! Read-only queries and traversal helpers over a [`TaskGraph`].

use agent_core::{TaskId, TaskState};
use std::collections::BTreeSet;

use super::{AgentTask, TaskGraph, TaskStateCounts};

impl TaskGraph {
    /// Returns task IDs that are ready to execute: `Pending` tasks whose
    /// dependencies are all in a terminal-completed state, plus tasks
    /// already in `Ready` state.
    pub fn ready_tasks(&self) -> Vec<TaskId> {
        let completed: BTreeSet<String> = self
            .tasks
            .values()
            .filter(|task| task.state == TaskState::Completed)
            .map(|task| task.id.to_string())
            .collect();
        self.tasks
            .values()
            .filter(|task| {
                (task.state == TaskState::Pending || task.state == TaskState::Ready)
                    && task
                        .dependencies
                        .iter()
                        .all(|dep| completed.contains(&dep.to_string()))
            })
            .map(|task| task.id.clone())
            .collect()
    }

    /// Find all transitive dependents of a task that would be blocked
    /// if this task fails (BlockDependents policy).
    pub fn find_blocked_dependents(&self, id: &TaskId) -> Vec<TaskId> {
        let mut blocked = Vec::new();
        let mut queue = vec![id.clone()];
        let mut visited = BTreeSet::new();

        while let Some(current) = queue.pop() {
            if visited.contains(&current.to_string()) {
                continue;
            }
            visited.insert(current.to_string());

            for task in self.tasks.values() {
                if task.dependencies.iter().any(|dep| dep == &current)
                    && !visited.contains(&task.id.to_string())
                {
                    blocked.push(task.id.clone());
                    queue.push(task.id.clone());
                }
            }
        }

        blocked
    }

    /// Find direct dependents of a task (tasks that list this task as a dependency).
    pub fn find_direct_dependents(&self, id: &TaskId) -> Vec<TaskId> {
        self.tasks
            .values()
            .filter(|task| task.dependencies.iter().any(|dep| dep == id))
            .map(|task| task.id.clone())
            .collect()
    }

    /// Get a reference to a task by ID.
    pub fn get_task(&self, id: &TaskId) -> Option<&AgentTask> {
        self.tasks.get(&id.to_string())
    }

    /// Get a mutable reference to a task by ID.
    pub fn get_task_mut(&mut self, id: &TaskId) -> Option<&mut AgentTask> {
        self.tasks.get_mut(&id.to_string())
    }

    /// Returns true if all tasks are in a terminal state (Completed, Failed, Skipped, or Cancelled).
    pub fn is_finished(&self) -> bool {
        !self.tasks.is_empty() && self.tasks.values().all(|task| task.state.is_terminal())
    }

    /// Return a snapshot of all tasks in the graph.
    pub fn snapshot(&self) -> Vec<AgentTask> {
        self.tasks.values().cloned().collect()
    }

    /// Returns the number of tasks in each state.
    pub fn state_counts(&self) -> TaskStateCounts {
        let mut counts = TaskStateCounts::default();
        for task in self.tasks.values() {
            match task.state {
                TaskState::Pending => counts.pending += 1,
                TaskState::Ready => counts.ready += 1,
                TaskState::Running => counts.running += 1,
                TaskState::Blocked => counts.blocked += 1,
                TaskState::Completed => counts.completed += 1,
                TaskState::Failed => counts.failed += 1,
                TaskState::Skipped => counts.skipped += 1,
                TaskState::Cancelled => counts.cancelled += 1,
            }
        }
        counts
    }
}

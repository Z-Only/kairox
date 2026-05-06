use agent_core::{AgentId, AgentRole, TaskFailureReason, TaskId, TaskState};
use std::collections::{BTreeMap, BTreeSet};

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

    pub fn mark_completed(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        task.state = TaskState::Completed;
        task.error = None;
        task.failure_reason = None;
        Ok(())
    }

    /// Mark a task as running. No-op if the task is already running or completed.
    /// Returns an error if the task ID is unknown.
    pub fn mark_running(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if task.state == TaskState::Pending || task.state == TaskState::Ready {
            task.state = TaskState::Running;
        }
        Ok(())
    }

    /// Mark a task as failed with an error message. Returns an error if the task ID is unknown.
    pub fn mark_failed(&mut self, id: &TaskId, error: String) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        task.state = TaskState::Failed;
        task.error = Some(error);
        Ok(())
    }

    /// Mark a task as failed with a structured failure reason.
    pub fn mark_failed_with_reason(
        &mut self,
        id: &TaskId,
        error: String,
        reason: TaskFailureReason,
    ) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        task.state = TaskState::Failed;
        task.error = Some(error);
        task.failure_reason = Some(reason);
        Ok(())
    }

    /// Mark a task as blocked because a dependency failed.
    pub fn mark_blocked(&mut self, id: &TaskId, reason: String) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if task.state == TaskState::Pending || task.state == TaskState::Ready {
            task.state = TaskState::Blocked;
            task.error = Some(reason);
        }
        Ok(())
    }

    /// Mark a task as skipped (user action to override a blocked/failed task).
    pub fn mark_skipped(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if !task.state.is_terminal()
            || task.state == TaskState::Failed
            || task.state == TaskState::Blocked
        {
            task.state = TaskState::Skipped;
            task.error = None;
            task.failure_reason = None;
        }
        Ok(())
    }

    /// Mark a task as cancelled.
    pub fn mark_cancelled(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if !task.state.is_terminal() {
            task.state = TaskState::Cancelled;
            task.error = Some("cancelled".into());
            task.failure_reason = Some(TaskFailureReason::Cancelled);
        }
        Ok(())
    }

    /// Reset a failed or blocked task back to pending (for retry).
    /// Increments retry_count. Returns error if the task is not in a retriable state.
    pub fn reset_to_pending(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if task.state == TaskState::Failed || task.state == TaskState::Blocked {
            task.state = TaskState::Pending;
            task.retry_count += 1;
            task.error = None;
            task.failure_reason = None;
            Ok(())
        } else {
            Err(crate::RuntimeError::UnknownTask(format!(
                "task {} is in state {:?}, cannot reset to pending (only Failed/Blocked allowed)",
                id, task.state
            )))
        }
    }

    /// Explicitly mark a task as ready (Pending → Ready).
    pub fn mark_ready(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if task.state == TaskState::Pending {
            task.state = TaskState::Ready;
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_ready_tasks_and_blocks_dependents() {
        let mut graph = TaskGraph::default();
        let plan = graph.add_task("plan", AgentRole::Planner, vec![]);
        let work = graph.add_task("work", AgentRole::Worker, vec![plan.clone()]);

        assert_eq!(graph.ready_tasks(), vec![plan.clone()]);
        graph.mark_completed(&plan).unwrap();
        assert_eq!(graph.ready_tasks(), vec![work]);
    }

    #[test]
    fn empty_graph_has_no_ready_tasks() {
        let graph = TaskGraph::default();
        assert!(graph.ready_tasks().is_empty());
    }

    #[test]
    fn independent_tasks_are_all_ready() {
        let mut graph = TaskGraph::default();
        let t1 = graph.add_task("task 1", AgentRole::Worker, vec![]);
        let t2 = graph.add_task("task 2", AgentRole::Worker, vec![]);

        let mut ready = graph.ready_tasks();
        ready.sort_by_key(|id| id.to_string());
        let mut expected = vec![t1, t2];
        expected.sort_by_key(|id| id.to_string());
        assert_eq!(ready, expected);
    }

    #[test]
    fn diamond_dependency_unblocks_after_all_parents_complete() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Planner, vec![]);
        let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
        let c = graph.add_task("C", AgentRole::Worker, vec![a.clone()]);
        let d = graph.add_task("D", AgentRole::Reviewer, vec![b.clone(), c.clone()]);

        assert_eq!(graph.ready_tasks(), vec![a.clone()]);

        graph.mark_completed(&a).unwrap();
        let mut ready = graph.ready_tasks();
        ready.sort_by_key(|id| id.to_string());
        let mut expected = vec![b.clone(), c.clone()];
        expected.sort_by_key(|id| id.to_string());
        assert_eq!(ready, expected);

        graph.mark_completed(&b).unwrap();
        assert_eq!(graph.ready_tasks(), vec![c.clone()]);

        graph.mark_completed(&c).unwrap();
        assert_eq!(graph.ready_tasks(), vec![d]);
    }

    #[test]
    fn mark_completed_unknown_task_returns_error() {
        let mut graph = TaskGraph::default();
        let unknown_id = TaskId::new();
        let result = graph.mark_completed(&unknown_id);
        assert!(result.is_err());
    }

    #[test]
    fn partial_completion_only_unblocks_fully_resolved() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Planner, vec![]);
        let b = graph.add_task("B", AgentRole::Worker, vec![]);
        let c = graph.add_task("C", AgentRole::Reviewer, vec![a.clone(), b.clone()]);

        assert_eq!(graph.ready_tasks().len(), 2);

        graph.mark_completed(&a).unwrap();
        assert_eq!(graph.ready_tasks(), vec![b.clone()]);

        graph.mark_completed(&b).unwrap();
        assert_eq!(graph.ready_tasks(), vec![c]);
    }

    #[test]
    fn multiple_tasks_share_dependency() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Planner, vec![]);
        let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
        let c = graph.add_task("C", AgentRole::Worker, vec![a.clone()]);

        assert_eq!(graph.ready_tasks(), vec![a.clone()]);

        graph.mark_completed(&a).unwrap();
        let mut ready = graph.ready_tasks();
        ready.sort_by_key(|id| id.to_string());
        let mut expected = vec![b, c];
        expected.sort_by_key(|id| id.to_string());
        assert_eq!(ready, expected);
    }

    #[test]
    fn mark_running_transitions_pending_to_running() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("task", AgentRole::Worker, vec![]);
        assert_eq!(
            graph.tasks.get(&id.to_string()).unwrap().state,
            TaskState::Pending
        );
        graph.mark_running(&id).unwrap();
        assert_eq!(
            graph.tasks.get(&id.to_string()).unwrap().state,
            TaskState::Running
        );
    }

    #[test]
    fn mark_running_is_idempotent() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("task", AgentRole::Worker, vec![]);
        graph.mark_running(&id).unwrap();
        graph.mark_running(&id).unwrap();
        assert_eq!(
            graph.tasks.get(&id.to_string()).unwrap().state,
            TaskState::Running
        );
    }

    #[test]
    fn mark_failed_transitions_to_failed_with_error() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("task", AgentRole::Worker, vec![]);
        graph.mark_running(&id).unwrap();
        graph.mark_failed(&id, "something broke".into()).unwrap();
        let task = graph.tasks.get(&id.to_string()).unwrap();
        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(task.error, Some("something broke".into()));
    }

    #[test]
    fn mark_failed_on_unknown_task_returns_error() {
        let mut graph = TaskGraph::default();
        let unknown = TaskId::new();
        let result = graph.mark_failed(&unknown, "err".into());
        assert!(result.is_err());
    }

    #[test]
    fn snapshot_returns_all_tasks() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Planner, vec![]);
        let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
        graph.mark_running(&a).unwrap();
        let snap = graph.snapshot();
        assert_eq!(snap.len(), 2);
        let a_snap = snap.iter().find(|t| t.id == a).unwrap();
        assert_eq!(a_snap.state, TaskState::Running);
        assert_eq!(a_snap.role, AgentRole::Planner);
        let b_snap = snap.iter().find(|t| t.id == b).unwrap();
        assert_eq!(b_snap.state, TaskState::Pending);
        assert_eq!(b_snap.dependencies, vec![a]);
    }

    // --- New Phase 2 tests ---

    #[test]
    fn mark_blocked_transitions_pending_to_blocked() {
        let mut graph = TaskGraph::default();
        let parent = graph.add_task("parent", AgentRole::Planner, vec![]);
        let child = graph.add_task("child", AgentRole::Worker, vec![parent.clone()]);
        graph.mark_failed(&parent, "parent failed".into()).unwrap();
        graph
            .mark_blocked(&child, "dependency failed".into())
            .unwrap();
        let task = graph.get_task(&child).unwrap();
        assert_eq!(task.state, TaskState::Blocked);
        assert_eq!(task.error, Some("dependency failed".into()));
    }

    #[test]
    fn mark_skipped_overrides_failed() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("task", AgentRole::Worker, vec![]);
        graph.mark_running(&id).unwrap();
        graph.mark_failed(&id, "error".into()).unwrap();
        graph.mark_skipped(&id).unwrap();
        let task = graph.get_task(&id).unwrap();
        assert_eq!(task.state, TaskState::Skipped);
        assert!(task.error.is_none());
    }

    #[test]
    fn mark_skipped_overrides_blocked() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("child", AgentRole::Worker, vec![]);
        graph.mark_blocked(&id, "dep failed".into()).unwrap();
        graph.mark_skipped(&id).unwrap();
        let task = graph.get_task(&id).unwrap();
        assert_eq!(task.state, TaskState::Skipped);
    }

    #[test]
    fn mark_cancelled_from_running() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("task", AgentRole::Worker, vec![]);
        graph.mark_running(&id).unwrap();
        graph.mark_cancelled(&id).unwrap();
        let task = graph.get_task(&id).unwrap();
        assert_eq!(task.state, TaskState::Cancelled);
    }

    #[test]
    fn reset_to_pending_from_failed() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("task", AgentRole::Worker, vec![]);
        graph.mark_running(&id).unwrap();
        graph.mark_failed(&id, "error".into()).unwrap();
        graph.reset_to_pending(&id).unwrap();
        let task = graph.get_task(&id).unwrap();
        assert_eq!(task.state, TaskState::Pending);
        assert_eq!(task.retry_count, 1);
        assert!(task.error.is_none());
    }

    #[test]
    fn reset_to_pending_from_blocked() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("child", AgentRole::Worker, vec![]);
        graph.mark_blocked(&id, "dep failed".into()).unwrap();
        graph.reset_to_pending(&id).unwrap();
        let task = graph.get_task(&id).unwrap();
        assert_eq!(task.state, TaskState::Pending);
        assert_eq!(task.retry_count, 1);
    }

    #[test]
    fn reset_to_pending_from_running_fails() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("task", AgentRole::Worker, vec![]);
        graph.mark_running(&id).unwrap();
        let result = graph.reset_to_pending(&id);
        assert!(result.is_err());
    }

    #[test]
    fn mark_ready_transitions_pending_to_ready() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("task", AgentRole::Worker, vec![]);
        graph.mark_ready(&id).unwrap();
        let task = graph.get_task(&id).unwrap();
        assert_eq!(task.state, TaskState::Ready);
    }

    #[test]
    fn find_blocked_dependents_cascades() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Planner, vec![]);
        let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
        let c = graph.add_task("C", AgentRole::Worker, vec![a.clone()]);
        let d = graph.add_task("D", AgentRole::Reviewer, vec![b.clone(), c.clone()]);

        let dependents = graph.find_blocked_dependents(&a);
        let mut dep_ids: Vec<String> = dependents.iter().map(|id| id.to_string()).collect();
        dep_ids.sort();
        assert_eq!(dep_ids.len(), 3);
        assert!(dep_ids.contains(&b.to_string()));
        assert!(dep_ids.contains(&c.to_string()));
        assert!(dep_ids.contains(&d.to_string()));
    }

    #[test]
    fn find_direct_dependents() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Planner, vec![]);
        let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
        let c = graph.add_task("C", AgentRole::Worker, vec![a.clone()]);
        let _d = graph.add_task("D", AgentRole::Reviewer, vec![b.clone()]);

        let direct = graph.find_direct_dependents(&a);
        let mut direct_ids: Vec<String> = direct.iter().map(|id| id.to_string()).collect();
        direct_ids.sort();
        assert_eq!(direct_ids.len(), 2);
        assert!(direct_ids.contains(&b.to_string()));
        assert!(direct_ids.contains(&c.to_string()));
    }

    #[test]
    fn get_task_returns_correct_task() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("my task", AgentRole::Worker, vec![]);
        let task = graph.get_task(&id).unwrap();
        assert_eq!(task.title, "my task");
        assert_eq!(task.role, AgentRole::Worker);
    }

    #[test]
    fn get_task_returns_none_for_unknown() {
        let graph = TaskGraph::default();
        assert!(graph.get_task(&TaskId::new()).is_none());
    }

    #[test]
    fn is_finished_all_terminal() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Worker, vec![]);
        let b = graph.add_task("B", AgentRole::Worker, vec![]);
        graph.mark_completed(&a).unwrap();
        graph.mark_failed(&b, "error".into()).unwrap();
        assert!(graph.is_finished());
    }

    #[test]
    fn is_finished_not_all_terminal() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Worker, vec![]);
        let _b = graph.add_task("B", AgentRole::Worker, vec![]);
        graph.mark_completed(&a).unwrap();
        // b is still Pending
        assert!(!graph.is_finished());
    }

    #[test]
    fn is_finished_empty_graph_is_false() {
        let graph = TaskGraph::default();
        assert!(!graph.is_finished());
    }

    #[test]
    fn add_task_with_config_sets_fields() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task_with_config(
            "configured task",
            AgentRole::Worker,
            vec![],
            5,
            Some(AgentId::worker("w1")),
        );
        let task = graph.get_task(&id).unwrap();
        assert_eq!(task.title, "configured task");
        assert_eq!(task.max_retries, 5);
        assert_eq!(task.assigned_agent_id, Some(AgentId::worker("w1")));
        assert_eq!(task.retry_count, 0);
    }

    #[test]
    fn mark_failed_with_reason() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("task", AgentRole::Worker, vec![]);
        graph.mark_running(&id).unwrap();
        graph
            .mark_failed_with_reason(
                &id,
                "tool failed".into(),
                TaskFailureReason::PermissionDenied {
                    tool_id: "shell.exec".into(),
                },
            )
            .unwrap();
        let task = graph.get_task(&id).unwrap();
        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(task.error, Some("tool failed".into()));
        assert!(matches!(
            task.failure_reason,
            Some(TaskFailureReason::PermissionDenied { .. })
        ));
    }

    #[test]
    fn full_state_machine_transition_sequence() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Planner, vec![]);
        let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);

        // A is pending and ready
        assert_eq!(graph.get_task(&a).unwrap().state, TaskState::Pending);
        assert!(graph.ready_tasks().contains(&a));

        // A runs and completes
        graph.mark_running(&a).unwrap();
        assert_eq!(graph.get_task(&a).unwrap().state, TaskState::Running);
        graph.mark_completed(&a).unwrap();

        // B becomes ready now
        assert!(graph.ready_tasks().contains(&b));
        graph.mark_running(&b).unwrap();

        // B fails
        graph.mark_failed(&b, "error".into()).unwrap();

        // Reset B for retry
        graph.reset_to_pending(&b).unwrap();
        assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Pending);
        assert_eq!(graph.get_task(&b).unwrap().retry_count, 1);

        // B runs again and completes
        graph.mark_running(&b).unwrap();
        graph.mark_completed(&b).unwrap();

        assert!(graph.is_finished());
    }

    #[test]
    fn failure_cascade_blocks_dependents() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Planner, vec![]);
        let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
        let c = graph.add_task("C", AgentRole::Reviewer, vec![b.clone()]);

        // A fails
        graph.mark_running(&a).unwrap();
        graph.mark_failed(&a, "planner error".into()).unwrap();

        // Cascade: mark B and C as blocked
        let dependents = graph.find_blocked_dependents(&a);
        for dep in &dependents {
            graph.mark_blocked(dep, "dependency failed".into()).unwrap();
        }

        assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Blocked);
        assert_eq!(graph.get_task(&c).unwrap().state, TaskState::Blocked);
    }

    #[test]
    fn skip_blocked_task_then_continue() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Planner, vec![]);
        let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);

        graph.mark_failed(&a, "error".into()).unwrap();
        graph.mark_blocked(&b, "dep failed".into()).unwrap();

        // Skip B
        graph.mark_skipped(&b).unwrap();
        assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Skipped);

        // Graph is not finished (A is Failed, B is Skipped — both terminal)
        assert!(graph.is_finished());
    }

    #[test]
    fn state_counts_tracks_all_states() {
        let mut graph = TaskGraph::default();
        let a = graph.add_task("A", AgentRole::Worker, vec![]);
        let b = graph.add_task("B", AgentRole::Worker, vec![]);
        let c = graph.add_task("C", AgentRole::Worker, vec![]);
        let d = graph.add_task("D", AgentRole::Worker, vec![]);

        graph.mark_running(&a).unwrap();
        graph.mark_completed(&b).unwrap();
        graph.mark_failed(&c, "err".into()).unwrap();
        graph.mark_blocked(&d, "blocked".into()).unwrap();

        let counts = graph.state_counts();
        assert_eq!(counts.pending, 0); // a is running, d is blocked
        assert_eq!(counts.running, 1);
        assert_eq!(counts.completed, 1);
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.blocked, 1);
    }

    #[test]
    fn cancel_non_terminal_task() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("task", AgentRole::Worker, vec![]);
        graph.mark_ready(&id).unwrap();
        graph.mark_cancelled(&id).unwrap();
        assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Cancelled);
    }

    #[test]
    fn cancel_completed_task_is_noop() {
        let mut graph = TaskGraph::default();
        let id = graph.add_task("task", AgentRole::Worker, vec![]);
        graph.mark_running(&id).unwrap();
        graph.mark_completed(&id).unwrap();
        graph.mark_cancelled(&id).unwrap(); // should be no-op
        assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Completed);
    }
}

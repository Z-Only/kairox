use agent_core::{AgentRole, TaskId, TaskState};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTask {
    pub id: TaskId,
    pub title: String,
    pub role: AgentRole,
    pub state: TaskState,
    pub dependencies: Vec<TaskId>,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
pub struct TaskGraph {
    tasks: BTreeMap<String, AgentTask>,
}

impl TaskGraph {
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
            role,
            state: TaskState::Pending,
            dependencies,
            error: None,
        };
        self.tasks.insert(id.to_string(), task);
        id
    }

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
                task.state == TaskState::Pending
                    && task
                        .dependencies
                        .iter()
                        .all(|dependency| completed.contains(&dependency.to_string()))
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
        Ok(())
    }

    /// Mark a task as running. No-op if the task is already running or completed.
    /// Returns an error if the task ID is unknown.
    pub fn mark_running(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        if task.state == TaskState::Pending {
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

    /// Return a snapshot of all tasks in the graph.
    pub fn snapshot(&self) -> Vec<AgentTask> {
        self.tasks.values().cloned().collect()
    }
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
}

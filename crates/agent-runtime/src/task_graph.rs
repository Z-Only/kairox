use agent_core::TaskId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRole {
    Planner,
    Worker,
    Reviewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Running,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTask {
    pub id: TaskId,
    pub title: String,
    pub role: AgentRole,
    pub state: TaskState,
    pub dependencies: Vec<TaskId>,
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
}

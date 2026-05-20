//! Integration-style coverage for TaskGraph scheduling and state transitions.

use agent_core::{AgentRole, TaskState};
use agent_runtime::TaskGraph;

#[test]
fn task_graph_linear_dag_scheduling() {
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Reviewer, vec![b.clone()]);

    assert_eq!(graph.ready_tasks(), vec![a.clone()]);

    graph.mark_completed(&a).unwrap();
    assert_eq!(graph.ready_tasks(), vec![b.clone()]);

    graph.mark_completed(&b).unwrap();
    assert_eq!(graph.ready_tasks(), vec![c]);

    graph.mark_completed(&graph.ready_tasks()[0]).unwrap();
    assert!(graph.is_finished());
}

#[test]
fn task_graph_parallel_dag_scheduling() {
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Worker, vec![a.clone()]);
    let d = graph.add_task("D", AgentRole::Reviewer, vec![b.clone(), c.clone()]);

    assert_eq!(graph.ready_tasks(), vec![a.clone()]);

    graph.mark_completed(&a).unwrap();
    let mut ready = graph.ready_tasks();
    ready.sort_by_key(|id| id.to_string());
    assert_eq!(ready.len(), 2);

    graph.mark_completed(&b).unwrap();
    assert_eq!(graph.ready_tasks(), vec![c.clone()]);

    graph.mark_completed(&c).unwrap();
    assert_eq!(graph.ready_tasks(), vec![d]);
}

#[test]
fn task_graph_diamond_dag() {
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Worker, vec![a.clone()]);
    let d = graph.add_task("D", AgentRole::Reviewer, vec![b.clone(), c.clone()]);

    assert_eq!(graph.ready_tasks(), vec![a.clone()]);

    graph.mark_completed(&a).unwrap();
    let mut ready = graph.ready_tasks();
    ready.sort_by_key(|id| id.to_string());
    assert_eq!(ready.len(), 2);

    graph.mark_completed(&b).unwrap();
    assert!(!graph.ready_tasks().contains(&d));

    graph.mark_completed(&c).unwrap();
    assert_eq!(graph.ready_tasks(), vec![d]);
}

#[test]
fn task_graph_failure_cascade_block_dependents() {
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Reviewer, vec![b.clone()]);

    graph.mark_running(&a).unwrap();
    graph.mark_failed(&a, "planner error".into()).unwrap();

    let dependents = graph.find_blocked_dependents(&a);
    for dep_id in &dependents {
        graph
            .mark_blocked(dep_id, format!("dependency {} failed", a))
            .unwrap();
    }

    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Blocked);
    assert_eq!(graph.get_task(&c).unwrap().state, TaskState::Blocked);
    assert!(
        !graph.is_finished(),
        "Blocked tasks are not terminal, graph should not be finished"
    );
}

#[test]
fn task_graph_skip_unblocks_dependents() {
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);

    graph.mark_failed(&a, "error".into()).unwrap();
    graph.mark_blocked(&b, "dependency failed".into()).unwrap();
    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Blocked);

    graph.mark_skipped(&b).unwrap();
    assert_eq!(graph.get_task(&b).unwrap().state, TaskState::Skipped);
    assert!(graph.is_finished());
}

#[test]
fn task_graph_retry_resets_to_pending() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("task", AgentRole::Worker, vec![]);

    graph.mark_running(&id).unwrap();
    graph.mark_failed(&id, "transient error".into()).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Failed);
    assert_eq!(graph.get_task(&id).unwrap().retry_count, 0);

    graph.reset_to_pending(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Pending);
    assert_eq!(graph.get_task(&id).unwrap().retry_count, 1);
    assert!(graph.get_task(&id).unwrap().error.is_none());
}

#[test]
fn task_graph_cancel_non_terminal() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("task", AgentRole::Worker, vec![]);

    graph.mark_ready(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Ready);

    graph.mark_cancelled(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Cancelled);
}

#[test]
fn task_graph_empty_graph_not_finished() {
    let graph = TaskGraph::default();
    assert!(!graph.is_finished(), "Empty graph should not be finished");
}

#[test]
fn task_graph_single_task_lifecycle() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("Only task", AgentRole::Worker, vec![]);

    assert!(graph.ready_tasks().contains(&id));
    assert!(!graph.is_finished());

    graph.mark_running(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Running);
    assert!(!graph.is_finished());

    graph.mark_completed(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Completed);
    assert!(graph.is_finished());
}

#[test]
fn task_graph_cancel_from_running_state() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("task", AgentRole::Worker, vec![]);
    graph.mark_running(&id).unwrap();
    graph.mark_cancelled(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Cancelled);
}

#[test]
fn task_graph_cancel_completed_is_noop() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("task", AgentRole::Worker, vec![]);
    graph.mark_running(&id).unwrap();
    graph.mark_completed(&id).unwrap();
    graph.mark_cancelled(&id).unwrap();
    assert_eq!(graph.get_task(&id).unwrap().state, TaskState::Completed);
}

#[test]
fn task_graph_skip_overrides_failed() {
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
fn task_graph_reset_to_pending_from_blocked() {
    let mut graph = TaskGraph::default();
    let id = graph.add_task("child", AgentRole::Worker, vec![]);
    graph.mark_blocked(&id, "dep failed".into()).unwrap();
    graph.reset_to_pending(&id).unwrap();
    let task = graph.get_task(&id).unwrap();
    assert_eq!(task.state, TaskState::Pending);
    assert_eq!(task.retry_count, 1);
}

#[test]
fn task_graph_state_counts_after_mixed_operations() {
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Worker, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![]);
    let c = graph.add_task("C", AgentRole::Worker, vec![]);
    let d = graph.add_task("D", AgentRole::Worker, vec![]);
    let e = graph.add_task("E", AgentRole::Worker, vec![]);

    graph.mark_completed(&a).unwrap();
    graph.mark_running(&b).unwrap();
    graph.mark_failed(&c, "error".into()).unwrap();
    graph.mark_blocked(&d, "blocked".into()).unwrap();
    graph.mark_skipped(&e).unwrap();

    let counts = graph.state_counts();
    assert_eq!(counts.completed, 1);
    assert_eq!(counts.running, 1);
    assert_eq!(counts.failed, 1);
    assert_eq!(counts.blocked, 1);
    assert_eq!(counts.skipped, 1);
    assert_eq!(counts.pending, 0);
}

#[test]
fn task_graph_find_blocked_dependents_cascades_transitively() {
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Planner, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);
    let c = graph.add_task("C", AgentRole::Worker, vec![b.clone()]);
    let d = graph.add_task("D", AgentRole::Reviewer, vec![c.clone()]);

    let dependents = graph.find_blocked_dependents(&a);
    assert_eq!(dependents.len(), 3, "B, C, D should all be dependents of A");

    let dep_ids: Vec<String> = dependents.iter().map(|id| id.to_string()).collect();
    assert!(dep_ids.contains(&b.to_string()));
    assert!(dep_ids.contains(&c.to_string()));
    assert!(dep_ids.contains(&d.to_string()));
}

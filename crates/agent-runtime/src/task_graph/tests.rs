use agent_core::{AgentId, AgentRole, TaskFailureReason, TaskId, TaskState};

use super::TaskGraph;

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

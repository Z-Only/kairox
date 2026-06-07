use super::*;

#[test]
fn next_execution_batch_respects_configured_concurrency() {
    let mut graph = TaskGraph::default();
    let first = graph.add_task("first", AgentRole::Worker, vec![]);
    let second = graph.add_task("second", AgentRole::Worker, vec![]);
    let third = graph.add_task("third", AgentRole::Worker, vec![]);

    let batch = next_execution_batch(&graph, 2);

    let mut expected = vec![first, second, third];
    expected.sort_by_key(|id| id.to_string());
    expected.truncate(2);
    assert_eq!(batch.task_ids, expected);
    assert_eq!(batch.capacity, 2);
}

#[test]
fn next_execution_batch_empty_graph() {
    let graph = TaskGraph::default();
    let batch = next_execution_batch(&graph, 4);
    assert!(batch.task_ids.is_empty());
}

#[test]
fn next_execution_batch_single_task() {
    let mut graph = TaskGraph::default();
    let _task = graph.add_task("only", AgentRole::Worker, vec![]);

    let batch = next_execution_batch(&graph, 4);
    assert_eq!(batch.task_ids.len(), 1);
    assert!(batch.capacity >= 1);
}

#[test]
fn next_execution_batch_respects_dependencies() {
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Worker, vec![]);
    let _b = graph.add_task("B", AgentRole::Worker, vec![a.clone()]);

    // Only A is ready; B depends on A
    let batch = next_execution_batch(&graph, 10);
    assert_eq!(batch.task_ids, vec![a]);
}

#[test]
fn next_execution_batch_max_concurrency_one() {
    let mut graph = TaskGraph::default();
    let _a = graph.add_task("A", AgentRole::Worker, vec![]);
    let _b = graph.add_task("B", AgentRole::Worker, vec![]);
    let _c = graph.add_task("C", AgentRole::Worker, vec![]);

    let batch = next_execution_batch(&graph, 1);
    assert_eq!(batch.task_ids.len(), 1);
    assert_eq!(batch.capacity, 1);
}

#[test]
fn next_execution_batch_all_completed_returns_empty() {
    let mut graph = TaskGraph::default();
    let a = graph.add_task("A", AgentRole::Worker, vec![]);
    let b = graph.add_task("B", AgentRole::Worker, vec![]);

    graph.mark_running(&a).unwrap();
    graph.mark_completed(&a).unwrap();
    graph.mark_running(&b).unwrap();
    graph.mark_completed(&b).unwrap();

    let batch = next_execution_batch(&graph, 4);
    assert!(batch.task_ids.is_empty());
}

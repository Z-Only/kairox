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

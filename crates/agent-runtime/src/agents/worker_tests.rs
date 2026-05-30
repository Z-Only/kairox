use super::*;
use agent_core::TaskId;

#[test]
fn worker_strategy_has_worker_role() {
    let strategy = WorkerStrategy::new();
    assert_eq!(strategy.role(), AgentRole::Worker);
}

#[tokio::test]
async fn worker_builds_context_from_task() {
    let strategy = WorkerStrategy::new();
    let task = AgentTask {
        id: TaskId::new(),
        description: String::new(),
        title: "Write tests".into(),
        role: AgentRole::Worker,
        state: agent_core::TaskState::Pending,
        dependencies: vec![],
        error: None,
        retry_count: 0,
        max_retries: 2,
        assigned_agent_id: None,
        failure_reason: None,
    };
    let graph = TaskGraph::default();
    let messages = strategy.build_context(&task, &graph, &[]).await;
    assert_eq!(messages.len(), 1);
    assert!(messages[0].content.contains("Write tests"));
}

#[tokio::test]
async fn worker_process_tool_result_retries_on_error_within_limit() {
    let strategy = WorkerStrategy::new();
    let tool_call = ToolCall {
        id: "call_1".into(),
        name: "fs.read".into(),
        arguments: serde_json::json!({"path": "/tmp/test"}),
    };
    let action = strategy
        .process_tool_result(&tool_call, "Error: file not found", 0)
        .await;
    assert_eq!(action, ToolResultAction::Retry { max_retries: 2 });
}

#[tokio::test]
async fn worker_process_tool_result_continues_after_retry_limit() {
    let strategy = WorkerStrategy::new();
    let tool_call = ToolCall {
        id: "call_1".into(),
        name: "fs.read".into(),
        arguments: serde_json::json!({"path": "/tmp/test"}),
    };
    let action = strategy
        .process_tool_result(&tool_call, "Error: still failing", 2)
        .await;
    assert_eq!(action, ToolResultAction::Continue);
}

#[tokio::test]
async fn worker_process_tool_result_continues_on_success() {
    let strategy = WorkerStrategy::new();
    let tool_call = ToolCall {
        id: "call_1".into(),
        name: "fs.read".into(),
        arguments: serde_json::json!({"path": "/tmp/test"}),
    };
    let action = strategy
        .process_tool_result(&tool_call, "file contents here", 0)
        .await;
    assert_eq!(action, ToolResultAction::Continue);
}

#[tokio::test]
async fn worker_decide_responds_with_assistant_text() {
    let strategy = WorkerStrategy::new();
    let ctx = StepContext {
        session_id: agent_core::SessionId::new(),
        workspace_id: agent_core::WorkspaceId::from_string("test".to_string()),
        user_message: "do task".into(),
        source_agent_id: AgentId::worker("w1"),
    };
    let messages = vec![ModelMessage {
        role: "assistant".into(),
        content: "Task completed successfully".into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    }];
    let decision = strategy.decide(&ctx, messages).await;
    assert!(matches!(decision, AgentDecision::Respond(_)));
}

#[tokio::test]
async fn worker_decide_requests_model_when_no_assistant() {
    let strategy = WorkerStrategy::new();
    let ctx = StepContext {
        session_id: agent_core::SessionId::new(),
        workspace_id: agent_core::WorkspaceId::from_string("test".to_string()),
        user_message: "do task".into(),
        source_agent_id: AgentId::worker("w1"),
    };
    let messages = vec![ModelMessage {
        role: "user".into(),
        content: "please do this".into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    }];
    let decision = strategy.decide(&ctx, messages).await;
    assert!(matches!(decision, AgentDecision::RequestModel { .. }));
}

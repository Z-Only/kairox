use super::*;
use agent_core::facade::AgentSettingsView;
use agent_core::{EventPayload, PrivacyClassification, TaskId};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_task(title: &str) -> AgentTask {
    AgentTask {
        id: TaskId::new(),
        description: String::new(),
        title: title.into(),
        role: AgentRole::Worker,
        state: agent_core::TaskState::Pending,
        dependencies: vec![],
        error: None,
        retry_count: 0,
        max_retries: 2,
        assigned_agent_id: None,
        failure_reason: None,
    }
}

fn make_tool_call() -> ToolCall {
    ToolCall {
        id: "call_1".into(),
        name: "fs.read".into(),
        arguments: serde_json::json!({"path": "/tmp/test"}),
    }
}

fn make_step_ctx() -> StepContext {
    StepContext {
        session_id: agent_core::SessionId::new(),
        workspace_id: agent_core::WorkspaceId::from_string("test".to_string()),
        user_message: "do task".into(),
        source_agent_id: AgentId::worker("w1"),
    }
}

fn make_domain_event(payload: EventPayload) -> agent_core::DomainEvent {
    agent_core::DomainEvent::new(
        agent_core::WorkspaceId::from_string("test".to_string()),
        agent_core::SessionId::new(),
        AgentId::worker("w1"),
        PrivacyClassification::MinimalTrace,
        payload,
    )
}

fn default_agent_view() -> AgentSettingsView {
    AgentSettingsView {
        settings_id: "test-id".into(),
        name: "test-worker".into(),
        description: String::new(),
        scope: agent_core::facade::AgentSettingsScope::Builtin,
        path: String::new(),
        tools: vec![],
        model_profile: None,
        reasoning_effort: None,
        skills: vec![],
        nickname_candidates: vec![],
        enabled: true,
        instructions: String::new(),
        effective: true,
        shadowed_by: None,
        valid: true,
        validation_error: None,
        editable: false,
        deletable: false,
    }
}

// ---------------------------------------------------------------------------
// Original tests (refactored to use helpers)
// ---------------------------------------------------------------------------

#[test]
fn worker_strategy_has_worker_role() {
    let strategy = WorkerStrategy::new();
    assert_eq!(strategy.role(), AgentRole::Worker);
}

#[tokio::test]
async fn worker_builds_context_from_task() {
    let strategy = WorkerStrategy::new();
    let task = make_task("Write tests");
    let graph = TaskGraph::default();
    let messages = strategy.build_context(&task, &graph, &[]).await;
    assert_eq!(messages.len(), 1);
    assert!(messages[0].content.contains("Write tests"));
}

#[tokio::test]
async fn worker_process_tool_result_retries_on_error_within_limit() {
    let strategy = WorkerStrategy::new();
    let action = strategy
        .process_tool_result(&make_tool_call(), "Error: file not found", 0)
        .await;
    assert_eq!(action, ToolResultAction::Retry { max_retries: 2 });
}

#[tokio::test]
async fn worker_process_tool_result_continues_after_retry_limit() {
    let strategy = WorkerStrategy::new();
    let action = strategy
        .process_tool_result(&make_tool_call(), "Error: still failing", 2)
        .await;
    assert_eq!(action, ToolResultAction::Continue);
}

#[tokio::test]
async fn worker_process_tool_result_continues_on_success() {
    let strategy = WorkerStrategy::new();
    let action = strategy
        .process_tool_result(&make_tool_call(), "file contents here", 0)
        .await;
    assert_eq!(action, ToolResultAction::Continue);
}

#[tokio::test]
async fn worker_decide_responds_with_assistant_text() {
    let strategy = WorkerStrategy::new();
    let ctx = make_step_ctx();
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
    let ctx = make_step_ctx();
    let messages = vec![ModelMessage {
        role: "user".into(),
        content: "please do this".into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    }];
    let decision = strategy.decide(&ctx, messages).await;
    assert!(matches!(decision, AgentDecision::RequestModel { .. }));
}

// ---------------------------------------------------------------------------
// from_agent_view
// ---------------------------------------------------------------------------

#[test]
fn from_agent_view_custom_instructions_override_default() {
    let mut view = default_agent_view();
    view.instructions = "Custom system prompt from user".into();
    let strategy = WorkerStrategy::from_agent_view(&view);
    assert_eq!(strategy.system_prompt, "Custom system prompt from user");
}

#[test]
fn from_agent_view_empty_instructions_fallback_to_default() {
    let view = default_agent_view(); // instructions is empty
    let strategy = WorkerStrategy::from_agent_view(&view);
    assert!(strategy.system_prompt.contains("You are a worker agent"));
}

#[test]
fn from_agent_view_whitespace_instructions_fallback_to_default() {
    let mut view = default_agent_view();
    view.instructions = "   \n\t  ".into();
    let strategy = WorkerStrategy::from_agent_view(&view);
    assert!(strategy.system_prompt.contains("You are a worker agent"));
}

#[test]
fn from_agent_view_passes_through_model_profile() {
    let mut view = default_agent_view();
    view.model_profile = Some("gpt-4o".into());
    let strategy = WorkerStrategy::from_agent_view(&view);
    assert_eq!(strategy.model_profile_override(), Some("gpt-4o"));
}

#[test]
fn from_agent_view_passes_through_reasoning_effort() {
    let mut view = default_agent_view();
    view.reasoning_effort = Some("high".into());
    let strategy = WorkerStrategy::from_agent_view(&view);
    assert_eq!(strategy.reasoning_effort_override(), Some("high"));
}

#[test]
fn from_agent_view_passes_through_skills() {
    let mut view = default_agent_view();
    view.skills = vec!["code-review".into(), "testing".into()];
    let strategy = WorkerStrategy::from_agent_view(&view);
    assert_eq!(strategy.skills(), &["code-review", "testing"]);
}

#[test]
fn from_agent_view_passes_through_tools_allowlist() {
    let mut view = default_agent_view();
    view.tools = vec!["fs.read".into(), "shell.exec".into()];
    let strategy = WorkerStrategy::from_agent_view(&view);
    assert_eq!(strategy.tools_allowlist(), &["fs.read", "shell.exec"]);
}

// ---------------------------------------------------------------------------
// with_system_prompt
// ---------------------------------------------------------------------------

#[test]
fn with_system_prompt_replaces_default() {
    let strategy = WorkerStrategy::new().with_system_prompt("My custom prompt");
    assert_eq!(strategy.system_prompt, "My custom prompt");
    assert!(!strategy.system_prompt.contains("You are a worker agent"));
}

// ---------------------------------------------------------------------------
// build_context -- dependencies
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_context_includes_dependency_info() {
    let strategy = WorkerStrategy::new();
    let mut graph = TaskGraph::default();
    let dep_id = graph.add_task("Setup environment", AgentRole::Worker, vec![]);

    let task = AgentTask {
        id: TaskId::new(),
        title: "Run tests".into(),
        description: String::new(),
        role: AgentRole::Worker,
        state: agent_core::TaskState::Pending,
        dependencies: vec![dep_id],
        error: None,
        retry_count: 0,
        max_retries: 2,
        assigned_agent_id: None,
        failure_reason: None,
    };

    let messages = strategy.build_context(&task, &graph, &[]).await;
    assert_eq!(messages.len(), 1);
    assert!(messages[0].content.contains("Dependency results:"));
    assert!(messages[0].content.contains("Setup environment"));
}

#[tokio::test]
async fn build_context_missing_dependency_excluded() {
    let strategy = WorkerStrategy::new();
    let graph = TaskGraph::default(); // empty graph

    let task = AgentTask {
        id: TaskId::new(),
        title: "Run tests".into(),
        description: String::new(),
        role: AgentRole::Worker,
        state: agent_core::TaskState::Pending,
        dependencies: vec![TaskId::new()], // non-existent dep
        error: None,
        retry_count: 0,
        max_retries: 2,
        assigned_agent_id: None,
        failure_reason: None,
    };

    let messages = strategy.build_context(&task, &graph, &[]).await;
    // dep not in graph -> dep_context stays empty -> no "Dependency results:" section
    assert!(!messages[0].content.contains("Dependency results:"));
}

// ---------------------------------------------------------------------------
// build_context -- session events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_context_includes_session_events() {
    let strategy = WorkerStrategy::new();
    let task = make_task("Do work");
    let graph = TaskGraph::default();

    let events = vec![make_domain_event(EventPayload::AssistantMessageCompleted {
        message_id: "m1".into(),
        content: "Previous assistant output".into(),
    })];

    let messages = strategy.build_context(&task, &graph, &events).await;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "Previous assistant output");
}

#[tokio::test]
async fn build_context_caps_session_events_at_ten() {
    let strategy = WorkerStrategy::new();
    let task = make_task("Do work");
    let graph = TaskGraph::default();

    let events: Vec<_> = (0..15)
        .map(|i| {
            make_domain_event(EventPayload::AssistantMessageCompleted {
                message_id: format!("m{i}"),
                content: format!("msg {i}"),
            })
        })
        .collect();

    let messages = strategy.build_context(&task, &graph, &events).await;
    // 1 user context message + at most 10 assistant messages
    assert_eq!(messages.len(), 11);
}

#[tokio::test]
async fn build_context_ignores_non_assistant_events() {
    let strategy = WorkerStrategy::new();
    let task = make_task("Do work");
    let graph = TaskGraph::default();

    let events = vec![make_domain_event(EventPayload::AgentTaskCompleted {
        task_id: TaskId::new(),
    })];

    let messages = strategy.build_context(&task, &graph, &events).await;
    // Only the user context message; the non-matching event is ignored
    assert_eq!(messages.len(), 1);
}

// ---------------------------------------------------------------------------
// build_context -- description
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_context_includes_description() {
    let strategy = WorkerStrategy::new();
    let mut task = make_task("Implement feature");
    task.description = "Add pagination support to the API".into();
    let graph = TaskGraph::default();

    let messages = strategy.build_context(&task, &graph, &[]).await;
    assert!(messages[0]
        .content
        .contains("Details: Add pagination support"));
}

#[tokio::test]
async fn build_context_omits_details_when_description_empty() {
    let strategy = WorkerStrategy::new();
    let task = make_task("Implement feature"); // description is empty
    let graph = TaskGraph::default();

    let messages = strategy.build_context(&task, &graph, &[]).await;
    assert!(!messages[0].content.contains("Details:"));
}

// ---------------------------------------------------------------------------
// decide -- tool_calls
// ---------------------------------------------------------------------------

#[tokio::test]
async fn decide_requests_model_when_assistant_has_tool_calls() {
    let strategy = WorkerStrategy::new();
    let ctx = make_step_ctx();
    let messages = vec![ModelMessage {
        role: "assistant".into(),
        content: "Let me check that.".into(),
        tool_calls: vec![ToolCall {
            id: "tc1".into(),
            name: "fs.read".into(),
            arguments: serde_json::json!({"path": "/tmp/x"}),
        }],
        tool_call_id: None,
    }];
    let decision = strategy.decide(&ctx, messages).await;
    assert!(matches!(decision, AgentDecision::RequestModel { .. }));
}

#[tokio::test]
async fn decide_requests_model_when_assistant_content_empty() {
    let strategy = WorkerStrategy::new();
    let ctx = make_step_ctx();
    let messages = vec![ModelMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    }];
    let decision = strategy.decide(&ctx, messages).await;
    // empty content + no tool_calls -> still RequestModel (content is empty)
    assert!(matches!(decision, AgentDecision::RequestModel { .. }));
}

// ---------------------------------------------------------------------------
// process_tool_result -- boundary
// ---------------------------------------------------------------------------

#[tokio::test]
async fn process_tool_result_retries_at_iteration_one() {
    let strategy = WorkerStrategy::new();
    let action = strategy
        .process_tool_result(&make_tool_call(), "Error: timeout", 1)
        .await;
    assert_eq!(action, ToolResultAction::Retry { max_retries: 2 });
}

#[tokio::test]
async fn process_tool_result_continues_at_iteration_two() {
    // iteration=2 is the boundary: condition `iteration < 2` is false
    let strategy = WorkerStrategy::new();
    let action = strategy
        .process_tool_result(&make_tool_call(), "Error: timeout", 2)
        .await;
    assert_eq!(action, ToolResultAction::Continue);
}

// ---------------------------------------------------------------------------
// process_tool_result -- non-"Error:" prefixes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn process_tool_result_lowercase_error_does_not_retry() {
    let strategy = WorkerStrategy::new();
    let action = strategy
        .process_tool_result(&make_tool_call(), "error: lowercase prefix", 0)
        .await;
    assert_eq!(action, ToolResultAction::Continue);
}

#[tokio::test]
async fn process_tool_result_failed_prefix_does_not_retry() {
    let strategy = WorkerStrategy::new();
    let action = strategy
        .process_tool_result(&make_tool_call(), "Failed: something broke", 0)
        .await;
    assert_eq!(action, ToolResultAction::Continue);
}

// ---------------------------------------------------------------------------
// Override accessors
// ---------------------------------------------------------------------------

#[test]
fn model_profile_override_returns_none_by_default() {
    let strategy = WorkerStrategy::new();
    assert_eq!(strategy.model_profile_override(), None);
}

#[test]
fn reasoning_effort_override_returns_none_by_default() {
    let strategy = WorkerStrategy::new();
    assert_eq!(strategy.reasoning_effort_override(), None);
}

#[test]
fn skills_returns_empty_by_default() {
    let strategy = WorkerStrategy::new();
    assert!(strategy.skills().is_empty());
}

#[test]
fn tools_allowlist_returns_empty_by_default() {
    let strategy = WorkerStrategy::new();
    assert!(strategy.tools_allowlist().is_empty());
}

use super::*;

#[test]
fn parse_decomposition_with_valid_json() {
    let text = r#"Here's my plan:
```json
{
  "decompose": [
    {
      "title": "Read the codebase",
      "role": "Worker",
      "description": "Read and understand the existing code structure"
    },
    {
      "title": "Implement feature",
      "role": "Worker",
      "description": "Implement the requested feature"
    },
    {
      "title": "Review changes",
      "role": "Reviewer",
      "description": "Review all changes for quality"
    }
  ]
}
```"#;

    let result = PlannerStrategy::parse_decomposition(text);
    assert!(result.is_some());
    let tasks = result.unwrap();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].title, "Read the codebase");
    assert_eq!(tasks[0].role, AgentRole::Worker);
    assert_eq!(tasks[2].role, AgentRole::Reviewer);
}

#[test]
fn parse_decomposition_returns_none_for_plain_text() {
    let text = "This is a simple answer, no decomposition needed.";
    let result = PlannerStrategy::parse_decomposition(text);
    assert!(result.is_none());
}

#[test]
fn parse_decomposition_returns_none_for_empty_array() {
    let text = r#"{"decompose": []}"#;
    let result = PlannerStrategy::parse_decomposition(text);
    assert!(result.is_none());
}

#[tokio::test]
async fn planner_decide_decomposes_with_valid_json() {
    let strategy = PlannerStrategy::new();
    let ctx = StepContext {
        session_id: agent_core::SessionId::new(),
        workspace_id: agent_core::WorkspaceId::from_string("test".to_string()),
        user_message: "build a web app".into(),
        source_agent_id: AgentId::planner(),
    };
    let messages = vec![ModelMessage {
        role: "assistant".into(),
        content: r#"{"decompose": [{"title": "Setup project", "role": "Worker", "description": "init", "dependencies": []}]}"#.into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    }];
    let decision = strategy.decide(&ctx, messages).await;
    assert!(matches!(decision, AgentDecision::Decompose { .. }));
}

#[tokio::test]
async fn planner_decide_responds_to_plain_text() {
    let strategy = PlannerStrategy::new();
    let ctx = StepContext {
        session_id: agent_core::SessionId::new(),
        workspace_id: agent_core::WorkspaceId::from_string("test".to_string()),
        user_message: "hello".into(),
        source_agent_id: AgentId::planner(),
    };
    let messages = vec![ModelMessage {
        role: "assistant".into(),
        content: "Hello, how can I help?".into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    }];
    let decision = strategy.decide(&ctx, messages).await;
    assert!(matches!(decision, AgentDecision::Respond(_)));
}

#[tokio::test]
async fn planner_decide_defaults_to_respond_when_no_assistant() {
    let strategy = PlannerStrategy::new();
    let ctx = StepContext {
        session_id: agent_core::SessionId::new(),
        workspace_id: agent_core::WorkspaceId::from_string("test".to_string()),
        user_message: "hello".into(),
        source_agent_id: AgentId::planner(),
    };
    let messages = vec![ModelMessage {
        role: "user".into(),
        content: "test".into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    }];
    let decision = strategy.decide(&ctx, messages).await;
    assert!(matches!(decision, AgentDecision::Respond(_)));
}

#[tokio::test]
async fn from_agent_view_sets_model_profile_skills_tools_and_instructions() {
    let view = agent_core::facade::AgentSettingsView {
        settings_id: "Builtin:default".into(),
        name: "default".into(),
        description: "test".into(),
        scope: agent_core::facade::AgentSettingsScope::Builtin,
        path: "builtin://default".into(),
        tools: vec!["fs.read".into(), "search".into()],
        model_profile: Some("fast".into()),
        skills: vec!["kairox-dev-workflow".into()],
        nickname_candidates: vec!["Default".into()],
        enabled: true,
        instructions: "Custom planner instructions.".into(),
        effective: true,
        shadowed_by: None,
        valid: true,
        validation_error: None,
        editable: false,
        deletable: false,
    };

    let strategy = PlannerStrategy::from_agent_view(&view);

    assert_eq!(strategy.model_profile_override(), Some("fast"));
    assert_eq!(strategy.skills(), &["kairox-dev-workflow"]);
    assert_eq!(strategy.tools_allowlist(), &["fs.read", "search"]);

    let task = AgentTask {
        id: agent_core::TaskId::new(),
        title: "test".into(),
        description: String::new(),
        role: AgentRole::Planner,
        state: agent_core::TaskState::Pending,
        dependencies: vec![],
        error: None,
        retry_count: 0,
        max_retries: 2,
        assigned_agent_id: None,
        failure_reason: None,
    };
    let graph = TaskGraph::default();
    assert!(strategy.build_context(&task, &graph, &[]).await[0]
        .content
        .contains("Custom planner instructions."));
}

#[test]
fn from_agent_view_falls_back_to_default_prompt_when_instructions_empty() {
    let view = agent_core::facade::AgentSettingsView {
        settings_id: "Builtin:default".into(),
        name: "default".into(),
        description: "test".into(),
        scope: agent_core::facade::AgentSettingsScope::Builtin,
        path: "builtin://default".into(),
        tools: vec![],
        model_profile: None,
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
    };

    let strategy = PlannerStrategy::from_agent_view(&view);

    assert_eq!(strategy.model_profile_override(), None);
    assert!(strategy.skills().is_empty());
    assert!(strategy.tools_allowlist().is_empty());
}

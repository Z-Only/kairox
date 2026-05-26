use super::*;

/// Simple PlannerAgent (backward compatibility — see `PlannerStrategy` for the
/// full `AgentStrategy` implementation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerAgent;

/// System prompt used by the planner to decompose a user goal into sub-tasks.
const PLANNER_SYSTEM_PROMPT: &str = r#"You are a planning agent. Your job is to decompose the user's goal into a structured set of sub-tasks that can be executed in parallel where possible.

When the user's request is complex enough to benefit from decomposition, respond with a JSON array of sub-tasks in this exact format:

```json
{
  "decompose": [
    {
      "title": "Short task title",
      "role": "Worker",
      "description": "Detailed description of what this task should accomplish",
      "dependencies": []
    }
  ]
}
```

For simple questions that don't need decomposition, just respond normally with text.

Rules:
- Each sub-task must have a clear, actionable title and description
- Use "Worker" role for implementation tasks and "Reviewer" role for review tasks
- List dependencies by task title (tasks with no dependencies can run in parallel)
- Keep the number of sub-tasks reasonable (typically 2-6)
- Put a "Reviewer" task at the end that depends on all Worker tasks if review is needed
"#;

/// Full AgentStrategy implementation for the PlannerAgent.
pub struct PlannerStrategy {
    system_prompt: String,
    model_profile: Option<String>,
    skills: Vec<String>,
    tools_allowlist: Vec<String>,
}

impl PlannerStrategy {
    pub fn new() -> Self {
        Self {
            system_prompt: PLANNER_SYSTEM_PROMPT.to_string(),
            model_profile: None,
            skills: Vec::new(),
            tools_allowlist: Vec::new(),
        }
    }

    /// Construct a PlannerStrategy from an effective agent settings view.
    pub fn from_agent_view(view: &agent_core::facade::AgentSettingsView) -> Self {
        Self {
            system_prompt: if view.instructions.trim().is_empty() {
                PLANNER_SYSTEM_PROMPT.to_string()
            } else {
                view.instructions.clone()
            },
            model_profile: view.model_profile.clone(),
            skills: view.skills.clone(),
            tools_allowlist: view.tools.clone(),
        }
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Try to parse a decomposition response from the model output.
    pub fn parse_decomposition(text: &str) -> Option<Vec<SubTaskDef>> {
        // Try to find JSON in the response
        let text = text.trim();

        // Look for a JSON block with "decompose" key
        if let Some(start) = text.find("{") {
            if let Some(end) = text.rfind("}") {
                let json_str = &text[start..=end];
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(decompose) = parsed.get("decompose") {
                        if let Some(tasks) = decompose.as_array() {
                            let mut sub_tasks = Vec::new();
                            for task in tasks {
                                let title = task
                                    .get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Untitled task")
                                    .to_string();
                                let role_str = task
                                    .get("role")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Worker");
                                let role = match role_str {
                                    "Reviewer" => AgentRole::Reviewer,
                                    "Planner" => AgentRole::Planner,
                                    _ => AgentRole::Worker,
                                };
                                let description = task
                                    .get("description")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                // Dependencies are referenced by title; they will be
                                // resolved to TaskIds by the DagExecutor after all
                                // tasks are created.
                                sub_tasks.push(SubTaskDef {
                                    title,
                                    role,
                                    dependencies: Vec::new(), // resolved later
                                    description,
                                });
                            }
                            if !sub_tasks.is_empty() {
                                return Some(sub_tasks);
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

impl Default for PlannerStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentStrategy for PlannerStrategy {
    fn role(&self) -> AgentRole {
        AgentRole::Planner
    }

    async fn build_context(
        &self,
        task: &AgentTask,
        _graph: &TaskGraph,
        _session_events: &[DomainEvent],
    ) -> Vec<ModelMessage> {
        vec![ModelMessage {
            role: "user".into(),
            content: format!(
                "{}\n\nGoal: {}",
                self.system_prompt,
                if task.description.is_empty() {
                    &task.title
                } else {
                    &task.description
                }
            ),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }]
    }

    async fn decide(&self, _ctx: &StepContext, messages: Vec<ModelMessage>) -> AgentDecision {
        // The planner's decision is based on the last assistant message
        if let Some(last) = messages.iter().rev().find(|m| m.role == "assistant") {
            if let Some(sub_tasks) = Self::parse_decomposition(&last.content) {
                return AgentDecision::Decompose { sub_tasks };
            }
            return AgentDecision::Respond(last.content.clone());
        }
        AgentDecision::Respond("No response generated".into())
    }

    async fn process_tool_result(
        &self,
        _tool_call: &ToolCall,
        _result: &str,
        _iteration: usize,
    ) -> ToolResultAction {
        // Planner doesn't call tools
        ToolResultAction::Continue
    }

    fn model_profile_override(&self) -> Option<&str> {
        self.model_profile.as_deref()
    }

    fn skills(&self) -> &[String] {
        &self.skills
    }

    fn tools_allowlist(&self) -> &[String] {
        &self.tools_allowlist
    }
}

#[cfg(test)]
mod tests {
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
}

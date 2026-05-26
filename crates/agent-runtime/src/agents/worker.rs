use super::*;

/// Simple WorkerAgent (backward compatibility).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerAgent;

/// System prompt used by the worker to execute a specific task.
const WORKER_SYSTEM_PROMPT: &str = r#"You are a worker agent. Your job is to execute a specific task that has been assigned to you.

You have access to tools and should use them as needed to complete your task. When you have finished the task, provide a summary of what was accomplished.

If a tool call fails, you may retry with different parameters. If you cannot complete the task after reasonable attempts, report the failure clearly.
"#;

/// Full AgentStrategy implementation for the WorkerAgent.
pub struct WorkerStrategy {
    system_prompt: String,
    model_profile: Option<String>,
    skills: Vec<String>,
    tools_allowlist: Vec<String>,
}

impl WorkerStrategy {
    pub fn new() -> Self {
        Self {
            system_prompt: WORKER_SYSTEM_PROMPT.to_string(),
            model_profile: None,
            skills: Vec::new(),
            tools_allowlist: Vec::new(),
        }
    }

    /// Construct a WorkerStrategy from an effective agent settings view.
    pub fn from_agent_view(view: &agent_core::facade::AgentSettingsView) -> Self {
        Self {
            system_prompt: if view.instructions.trim().is_empty() {
                WORKER_SYSTEM_PROMPT.to_string()
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
}

impl Default for WorkerStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentStrategy for WorkerStrategy {
    fn role(&self) -> AgentRole {
        AgentRole::Worker
    }

    async fn build_context(
        &self,
        task: &AgentTask,
        graph: &TaskGraph,
        session_events: &[DomainEvent],
    ) -> Vec<ModelMessage> {
        let mut messages = Vec::new();

        // Build context from dependency outputs
        let mut dep_context = String::new();
        for dep_id in &task.dependencies {
            if let Some(dep) = graph.get_task(dep_id) {
                dep_context.push_str(&format!(
                    "- [{}] {} (state: {:?})\n",
                    dep.role, dep.title, dep.state
                ));
            }
        }

        let context = if dep_context.is_empty() {
            format!(
                "{}\n\nTask: {}\n{}",
                self.system_prompt,
                task.title,
                if task.description.is_empty() {
                    String::new()
                } else {
                    format!("\nDetails: {}", task.description)
                }
            )
        } else {
            format!(
                "{}\n\nTask: {}\n{}\n\nDependency results:\n{}",
                self.system_prompt,
                task.title,
                if task.description.is_empty() {
                    String::new()
                } else {
                    format!("\nDetails: {}", task.description)
                },
                dep_context
            )
        };

        messages.push(ModelMessage {
            role: "user".into(),
            content: context,
            tool_calls: Vec::new(),
            tool_call_id: None,
        });

        // Include recent session events as context
        for event in session_events.iter().rev().take(10) {
            if let agent_core::EventPayload::AssistantMessageCompleted { content, .. } =
                &event.payload
            {
                messages.push(ModelMessage {
                    role: "assistant".into(),
                    content: content.clone(),
                    tool_calls: Vec::new(),
                    tool_call_id: None,
                });
            }
        }

        messages
    }

    async fn decide(&self, _ctx: &StepContext, messages: Vec<ModelMessage>) -> AgentDecision {
        // Worker always requests model with tools
        if let Some(last) = messages.iter().rev().find(|m| m.role == "assistant") {
            if !last.content.is_empty() && last.tool_calls.is_empty() {
                // If the assistant provided a text-only response, treat it as a final response
                return AgentDecision::Respond(last.content.clone());
            }
        }
        // If there are tool calls or this is the first message, request the model
        AgentDecision::RequestModel { tools: Vec::new() }
    }

    async fn process_tool_result(
        &self,
        _tool_call: &ToolCall,
        result: &str,
        iteration: usize,
    ) -> ToolResultAction {
        // If the tool result contains an error and we haven't exceeded retries
        if result.starts_with("Error:") && iteration < 2 {
            ToolResultAction::Retry { max_retries: 2 }
        } else {
            ToolResultAction::Continue
        }
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
}

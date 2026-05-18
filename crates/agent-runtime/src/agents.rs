//! Agent strategy trait and built-in strategy implementations.
//!
//! The `AgentStrategy` trait defines how different agent roles (Planner, Worker,
//! Reviewer) interact with the DAG executor. Each strategy determines how to
//! build context, make decisions, and process tool results.

use agent_core::{AgentId, AgentRole, DomainEvent, TaskId};
use agent_models::{ModelMessage, ToolCall};

use async_trait::async_trait;

use crate::task_graph::{AgentTask, TaskGraph};

// Re-export the simple agents for backward compatibility.
pub use planner_agent::PlannerAgent;
pub use reviewer_agent::{ReviewerAgent, ReviewerFinding};
pub use worker_agent::WorkerAgent;

/// Context provided to a strategy for each step of execution.
#[derive(Debug, Clone)]
pub struct StepContext {
    pub session_id: agent_core::SessionId,
    pub workspace_id: agent_core::WorkspaceId,
    pub user_message: String,
    pub source_agent_id: AgentId,
}

/// Outcome of a single agent step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// The agent loop should continue (tool calls need processing).
    Continue,
    /// The agent has completed its task successfully.
    Completed,
    /// The agent was cancelled.
    Cancelled,
    /// A permission request is pending (interactive mode).
    PermissionRequired,
    /// Maximum iterations reached.
    MaxIterations,
}

/// Decision returned by an agent after processing a model response.
#[derive(Debug, Clone)]
pub enum AgentDecision {
    /// Request a model call with the given tool definitions.
    RequestModel {
        tools: Vec<agent_models::ToolDefinition>,
    },
    /// The agent has a final text response (no tool calls needed).
    Respond(String),
    /// The planner has decomposed the goal into sub-tasks.
    Decompose { sub_tasks: Vec<SubTaskDef> },
    /// The reviewer has completed its review.
    ReviewComplete {
        approved: bool,
        findings: Vec<ReviewerFinding>,
    },
}

/// Action to take after receiving a tool result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolResultAction {
    /// Continue the agent loop (feed the result to the model).
    Continue,
    /// Retry the tool call up to `max_retries` times.
    Retry { max_retries: usize },
    /// Abort the task with an error message.
    Abort(String),
}

/// Definition of a sub-task produced by the PlannerAgent during decomposition.
#[derive(Debug, Clone)]
pub struct SubTaskDef {
    pub title: String,
    pub role: AgentRole,
    pub dependencies: Vec<TaskId>,
    pub description: String,
}

/// The `AgentStrategy` trait defines how an agent role interacts with the DAG executor.
///
/// Each strategy is a stateless object that provides:
/// - Which role it serves (Planner, Worker, Reviewer)
/// - How to build the model context for a given task
/// - How to decide what to do with a model response
/// - How to process tool results
#[async_trait]
pub trait AgentStrategy: Send + Sync {
    /// The agent role this strategy implements.
    fn role(&self) -> AgentRole;

    /// Build the model messages for a given task, incorporating context from
    /// the task graph and session history.
    async fn build_context(
        &self,
        task: &AgentTask,
        graph: &TaskGraph,
        session_events: &[DomainEvent],
    ) -> Vec<ModelMessage>;

    /// Decide what to do given the current context and model messages.
    async fn decide(&self, ctx: &StepContext, messages: Vec<ModelMessage>) -> AgentDecision;

    /// Process the result of a tool call and determine the next action.
    async fn process_tool_result(
        &self,
        tool_call: &ToolCall,
        result: &str,
        iteration: usize,
    ) -> ToolResultAction;

    /// Optional model profile override from agent settings.
    fn model_profile_override(&self) -> Option<&str> {
        None
    }

    /// Optional permission mode override from agent settings.
    fn permission_mode_override(&self) -> Option<&str> {
        None
    }

    /// Skills configured for this agent.
    fn skills(&self) -> &[String] {
        &[]
    }

    /// Tool allowlist configured for this agent (empty = all allowed).
    fn tools_allowlist(&self) -> &[String] {
        &[]
    }
}

// Sub-modules for the three built-in strategies.

pub mod planner_agent {
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
        permission_mode: Option<String>,
        skills: Vec<String>,
        tools_allowlist: Vec<String>,
    }

    impl PlannerStrategy {
        pub fn new() -> Self {
            Self {
                system_prompt: PLANNER_SYSTEM_PROMPT.to_string(),
                model_profile: None,
                permission_mode: None,
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
                permission_mode: view.permission_mode.clone(),
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

        fn permission_mode_override(&self) -> Option<&str> {
            self.permission_mode.as_deref()
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
        async fn from_agent_view_sets_model_profile_permission_mode_skills_and_instructions() {
            let view = agent_core::facade::AgentSettingsView {
                settings_id: "Builtin:default".into(),
                name: "default".into(),
                description: "test".into(),
                scope: agent_core::facade::AgentSettingsScope::Builtin,
                path: "builtin://default".into(),
                tools: vec!["fs.read".into(), "search".into()],
                model_profile: Some("fast".into()),
                permission_mode: Some("read_only".into()),
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
            assert_eq!(strategy.permission_mode_override(), Some("read_only"));
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
                permission_mode: None,
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
            assert_eq!(strategy.permission_mode_override(), None);
            assert!(strategy.skills().is_empty());
            assert!(strategy.tools_allowlist().is_empty());
        }
    }
}

pub mod worker_agent {
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
        permission_mode: Option<String>,
        skills: Vec<String>,
        tools_allowlist: Vec<String>,
    }

    impl WorkerStrategy {
        pub fn new() -> Self {
            Self {
                system_prompt: WORKER_SYSTEM_PROMPT.to_string(),
                model_profile: None,
                permission_mode: None,
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
                permission_mode: view.permission_mode.clone(),
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

        fn permission_mode_override(&self) -> Option<&str> {
            self.permission_mode.as_deref()
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
}

pub mod reviewer_agent {
    use super::*;

    /// Simple ReviewerAgent (backward compatibility).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ReviewerAgent;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ReviewerFinding {
        pub severity: String,
        pub message: String,
    }

    impl ReviewerAgent {
        pub fn review_diff(diff: &str) -> Vec<ReviewerFinding> {
            let mut findings = Vec::new();
            if diff.contains("rm -rf") {
                findings.push(ReviewerFinding {
                    severity: "high".into(),
                    message: "destructive shell command requires explicit approval".into(),
                });
            }
            findings
        }
    }

    /// System prompt used by the reviewer to evaluate worker output.
    const REVIEWER_SYSTEM_PROMPT: &str = r#"You are a reviewer agent. Your job is to review the output of worker tasks and determine if the work meets quality standards.

Evaluate the work based on:
1. Completeness — does the output address the task requirements?
2. Correctness — are there any obvious errors or bugs?
3. Quality — is the code/output well-structured and maintainable?

Respond with a JSON object:

If approved:
```json
{"approved": true, "findings": []}
```

If issues found:
```json
{
  "approved": false,
  "findings": [
    {"severity": "high", "message": "Description of the issue"}
  ]
}
```
"#;

    /// Full AgentStrategy implementation for the ReviewerAgent.
    pub struct ReviewerStrategy {
        system_prompt: String,
        model_profile: Option<String>,
        permission_mode: Option<String>,
        skills: Vec<String>,
        tools_allowlist: Vec<String>,
    }

    impl ReviewerStrategy {
        pub fn new() -> Self {
            Self {
                system_prompt: REVIEWER_SYSTEM_PROMPT.to_string(),
                model_profile: None,
                permission_mode: None,
                skills: Vec::new(),
                tools_allowlist: Vec::new(),
            }
        }

        /// Construct a ReviewerStrategy from an effective agent settings view.
        pub fn from_agent_view(view: &agent_core::facade::AgentSettingsView) -> Self {
            Self {
                system_prompt: if view.instructions.trim().is_empty() {
                    REVIEWER_SYSTEM_PROMPT.to_string()
                } else {
                    view.instructions.clone()
                },
                model_profile: view.model_profile.clone(),
                permission_mode: view.permission_mode.clone(),
                skills: view.skills.clone(),
                tools_allowlist: view.tools.clone(),
            }
        }

        pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
            self.system_prompt = prompt.into();
            self
        }

        /// Try to parse a review response from the model output.
        pub fn parse_review(text: &str) -> (bool, Vec<ReviewerFinding>) {
            let text = text.trim();

            if let Some(start) = text.find("{") {
                if let Some(end) = text.rfind("}") {
                    let json_str = &text[start..=end];
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                        let approved = parsed
                            .get("approved")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let findings =
                            if let Some(arr) = parsed.get("findings").and_then(|v| v.as_array()) {
                                arr.iter()
                                    .filter_map(|f| {
                                        Some(ReviewerFinding {
                                            severity: f.get("severity")?.as_str()?.to_string(),
                                            message: f.get("message")?.as_str()?.to_string(),
                                        })
                                    })
                                    .collect()
                            } else {
                                Vec::new()
                            };
                        return (approved, findings);
                    }
                }
            }

            // Default: if we can't parse, assume approved
            (true, Vec::new())
        }
    }

    impl Default for ReviewerStrategy {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl AgentStrategy for ReviewerStrategy {
        fn role(&self) -> AgentRole {
            AgentRole::Reviewer
        }

        async fn build_context(
            &self,
            task: &AgentTask,
            graph: &TaskGraph,
            _session_events: &[DomainEvent],
        ) -> Vec<ModelMessage> {
            let mut dep_outputs = Vec::new();
            for dep_id in &task.dependencies {
                if let Some(dep) = graph.get_task(dep_id) {
                    dep_outputs.push(format!(
                        "[{}] {} (state: {:?})",
                        dep.role, dep.title, dep.state
                    ));
                }
            }

            let context = format!(
                "{}\n\nReview the following task and its dependency outputs:\n\nTask: {}\n{}\n\nDependency outputs:\n{}",
                self.system_prompt,
                task.title,
                if task.description.is_empty() {
                    String::new()
                } else {
                    format!("Details: {}", task.description)
                },
                if dep_outputs.is_empty() {
                    "(none)".to_string()
                } else {
                    dep_outputs.join("\n")
                }
            );

            vec![ModelMessage {
                role: "user".into(),
                content: context,
                tool_calls: Vec::new(),
                tool_call_id: None,
            }]
        }

        async fn decide(&self, _ctx: &StepContext, messages: Vec<ModelMessage>) -> AgentDecision {
            if let Some(last) = messages.iter().rev().find(|m| m.role == "assistant") {
                let (approved, findings) = Self::parse_review(&last.content);
                return AgentDecision::ReviewComplete { approved, findings };
            }
            AgentDecision::ReviewComplete {
                approved: true,
                findings: vec![],
            }
        }

        async fn process_tool_result(
            &self,
            _tool_call: &ToolCall,
            _result: &str,
            _iteration: usize,
        ) -> ToolResultAction {
            // Reviewer doesn't call tools
            ToolResultAction::Continue
        }

        fn model_profile_override(&self) -> Option<&str> {
            self.model_profile.as_deref()
        }

        fn permission_mode_override(&self) -> Option<&str> {
            self.permission_mode.as_deref()
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
        fn reviewer_flags_destructive_commands() {
            let findings = ReviewerAgent::review_diff("+ rm -rf target");
            assert_eq!(findings[0].severity, "high");
        }

        #[test]
        fn parse_review_approved() {
            let text = r#"```json
{"approved": true, "findings": []}
```"#;
            let (approved, findings) = ReviewerStrategy::parse_review(text);
            assert!(approved);
            assert!(findings.is_empty());
        }

        #[test]
        fn parse_review_with_findings() {
            let text = r#"{"approved": false, "findings": [{"severity": "high", "message": "Missing error handling"}]}"#;
            let (approved, findings) = ReviewerStrategy::parse_review(text);
            assert!(!approved);
            assert_eq!(findings.len(), 1);
            assert_eq!(findings[0].severity, "high");
        }

        #[test]
        fn parse_review_defaults_to_approved() {
            let text = "The code looks good overall.";
            let (approved, _) = ReviewerStrategy::parse_review(text);
            assert!(approved);
        }

        #[test]
        fn reviewer_strategy_has_reviewer_role() {
            let strategy = ReviewerStrategy::new();
            assert_eq!(strategy.role(), AgentRole::Reviewer);
        }
    }
}

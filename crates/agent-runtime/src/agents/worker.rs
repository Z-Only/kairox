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
    reasoning_effort: Option<String>,
    skills: Vec<String>,
    tools_allowlist: Vec<String>,
}

impl WorkerStrategy {
    pub fn new() -> Self {
        Self {
            system_prompt: WORKER_SYSTEM_PROMPT.to_string(),
            model_profile: None,
            reasoning_effort: None,
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
            reasoning_effort: view.reasoning_effort.clone(),
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

    fn reasoning_effort_override(&self) -> Option<&str> {
        self.reasoning_effort.as_deref()
    }

    fn skills(&self) -> &[String] {
        &self.skills
    }

    fn tools_allowlist(&self) -> &[String] {
        &self.tools_allowlist
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;

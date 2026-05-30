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
    reasoning_effort: Option<String>,
    skills: Vec<String>,
    tools_allowlist: Vec<String>,
}

impl PlannerStrategy {
    pub fn new() -> Self {
        Self {
            system_prompt: PLANNER_SYSTEM_PROMPT.to_string(),
            model_profile: None,
            reasoning_effort: None,
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
            reasoning_effort: view.reasoning_effort.clone(),
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
#[path = "planner_tests.rs"]
mod tests;

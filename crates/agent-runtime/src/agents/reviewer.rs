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
    reasoning_effort: Option<String>,
    skills: Vec<String>,
    tools_allowlist: Vec<String>,
}

impl ReviewerStrategy {
    pub fn new() -> Self {
        Self {
            system_prompt: REVIEWER_SYSTEM_PROMPT.to_string(),
            model_profile: None,
            reasoning_effort: None,
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
            reasoning_effort: view.reasoning_effort.clone(),
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

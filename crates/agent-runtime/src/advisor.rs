//! Advisor (self-reflection) — a secondary model reviews the primary
//! agent's planned tool calls before execution.
//!
//! The advisor is **not** an `AgentStrategy`; it runs inline inside the
//! agent loop between "model produced tool calls" and "tool calls are
//! executed." It makes a single model request, parses the structured
//! response, and returns an [`AdvisorReview`].

use agent_core::{AdvisorConcern, AdvisorMode, AdvisorReview, AdvisorVerdict};
use agent_models::{ModelClient, ModelMessage, ModelRequest};
use futures::StreamExt;

/// Should this batch of tool calls be reviewed by the advisor?
///
/// - `Off` → never
/// - `Full` → always
/// - `Lightweight` → only when at least one tool call is high-risk
pub fn should_review(mode: AdvisorMode, tool_calls: &[agent_models::ToolCall]) -> bool {
    match mode {
        AdvisorMode::Off => false,
        AdvisorMode::Full => true,
        AdvisorMode::Lightweight => tool_calls.iter().any(is_high_risk_tool_call),
    }
}

/// Heuristic for whether a tool call is "high-risk" and warrants
/// lightweight advisor review.
fn is_high_risk_tool_call(tool_call: &agent_models::ToolCall) -> bool {
    let name = tool_call.name.as_str();
    let args_lower = tool_call.arguments.to_string().to_lowercase();

    // Destructive shell commands
    if name == "shell.exec" || name == "shell" || name == "bash" {
        let dangerous_patterns = [
            "rm -rf",
            "rm -r",
            "rmdir",
            "mkfs",
            "dd if=",
            "format",
            "> /dev/",
            "chmod 777",
            "curl | sh",
            "wget | sh",
            "sudo",
            "kill -9",
            "pkill",
            "shutdown",
            "reboot",
        ];
        return dangerous_patterns
            .iter()
            .any(|pattern| args_lower.contains(pattern));
    }

    // File writes outside typical workspace patterns
    if name == "fs.write" || name == "fs.patch" || name == "patch.apply" {
        let sensitive_paths = [
            "/etc/",
            "/usr/",
            "/bin/",
            "/sbin/",
            "~/.ssh",
            "~/.config",
            ".env",
            "credentials",
            "secrets",
        ];
        return sensitive_paths.iter().any(|path| args_lower.contains(path));
    }

    // Computer use / desktop interaction
    if name == "computer.use" {
        return true;
    }

    false
}

/// System prompt for the advisor model.
const ADVISOR_SYSTEM_PROMPT: &str = r#"You are a safety advisor reviewing an AI agent's planned tool calls before execution. Your job is to identify potential risks, destructive operations, or mistakes.

Review the tool calls and respond with a JSON object:

If the plan is safe:
```json
{"verdict": "approve", "concerns": [], "summary": "Plan looks safe."}
```

If there are concerns but execution can proceed:
```json
{
  "verdict": "approve_with_warnings",
  "concerns": [
    {"tool_name": "shell.exec", "severity": "medium", "message": "Description of concern"}
  ],
  "summary": "Brief summary of warnings."
}
```

If execution should be blocked:
```json
{
  "verdict": "reject",
  "concerns": [
    {"tool_name": "shell.exec", "severity": "high", "message": "Why this is dangerous"}
  ],
  "summary": "Why execution should not proceed."
}
```

Severity levels: "high" (destructive/irreversible), "medium" (risky but recoverable), "low" (minor concern).

Focus on:
1. Destructive operations (rm -rf, format, overwrite critical files)
2. Security risks (credential exposure, privilege escalation)
3. Scope violations (modifying files outside the workspace)
4. Logic errors (wrong file path, incorrect arguments)
5. Resource exhaustion (infinite loops, unbounded operations)
"#;

/// Run the advisor review on a batch of tool calls.
///
/// Returns `None` if the model call fails (advisor failures should not
/// block the main agent loop).
pub async fn review_tool_calls<M: ModelClient>(
    model: &M,
    advisor_profile: &str,
    tool_calls: &[agent_models::ToolCall],
    assistant_text: &str,
    max_concerns: usize,
) -> Option<AdvisorReview> {
    let tool_call_descriptions: Vec<String> = tool_calls
        .iter()
        .map(|tc| format!("- **{}** ({})", tc.name, tc.arguments))
        .collect();

    let user_message = format!(
        "The AI agent plans to execute the following tool calls:\n\n{}\n\n\
         Agent's reasoning:\n{}\n\n\
         Please review these tool calls for safety and correctness.",
        tool_call_descriptions.join("\n"),
        if assistant_text.is_empty() {
            "(no reasoning provided)"
        } else {
            assistant_text
        }
    );

    let request = ModelRequest {
        model_profile: advisor_profile.to_string(),
        messages: vec![ModelMessage {
            role: "user".into(),
            content: user_message,
            tool_calls: Vec::new(),
            tool_call_id: None,
        }],
        system_prompt: Some(ADVISOR_SYSTEM_PROMPT.to_string()),
        tools: Vec::new(),
        server_tools: Vec::new(),
        reasoning_effort: None,
    };

    let mut stream = match model.stream(request).await {
        Ok(stream) => stream,
        Err(error) => {
            tracing::warn!(%error, "advisor model call failed — skipping review");
            return None;
        }
    };

    let mut response_text = String::new();
    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(agent_models::ModelEvent::TokenDelta(delta)) => response_text.push_str(&delta),
            Ok(agent_models::ModelEvent::Completed { .. }) => break,
            Ok(agent_models::ModelEvent::Failed { message }) => {
                tracing::warn!(%message, "advisor model stream failed — skipping review");
                return None;
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "advisor model stream error — skipping review");
                return None;
            }
        }
    }

    let mut review = parse_advisor_response(&response_text);
    review.advisor_profile = advisor_profile.to_string();
    review.concerns.truncate(max_concerns);
    Some(review)
}

/// Parse the advisor model's JSON response into an `AdvisorReview`.
///
/// Falls back to `Approve` with a note if parsing fails — we don't want
/// a malformed advisor response to block the main agent.
pub fn parse_advisor_response(text: &str) -> AdvisorReview {
    let text = text.trim();

    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            let json_str = &text[start..=end];
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                let verdict = parsed
                    .get("verdict")
                    .and_then(|v| v.as_str())
                    .map(parse_verdict)
                    .unwrap_or(AdvisorVerdict::Approve);

                let concerns = parsed
                    .get("concerns")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|concern| {
                                Some(AdvisorConcern {
                                    tool_name: concern
                                        .get("tool_name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string(),
                                    severity: concern
                                        .get("severity")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("medium")
                                        .to_string(),
                                    message: concern
                                        .get("message")
                                        .and_then(|v| v.as_str())?
                                        .to_string(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let summary = parsed
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                return AdvisorReview {
                    verdict,
                    concerns,
                    summary,
                    advisor_profile: String::new(),
                };
            }
        }
    }

    // Fallback: unparseable response → approve
    AdvisorReview {
        verdict: AdvisorVerdict::Approve,
        concerns: Vec::new(),
        summary: "Advisor response could not be parsed; defaulting to approve.".into(),
        advisor_profile: String::new(),
    }
}

fn parse_verdict(verdict_str: &str) -> AdvisorVerdict {
    match verdict_str {
        "approve" => AdvisorVerdict::Approve,
        "approve_with_warnings" => AdvisorVerdict::ApproveWithWarnings,
        "reject" => AdvisorVerdict::Reject,
        _ => AdvisorVerdict::Approve,
    }
}

#[cfg(test)]
#[path = "advisor_tests.rs"]
mod tests;

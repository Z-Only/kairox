use super::*;
use agent_core::AdvisorVerdict;
use agent_models::ToolCall;

// ── should_review ───────────────────────────────────────────────────

#[test]
fn off_mode_never_reviews() {
    let calls = vec![dangerous_shell_call()];
    assert!(!should_review(AdvisorMode::Off, &calls));
}

#[test]
fn full_mode_always_reviews() {
    let calls = vec![safe_read_call()];
    assert!(should_review(AdvisorMode::Full, &calls));
}

#[test]
fn lightweight_mode_reviews_high_risk() {
    assert!(should_review(
        AdvisorMode::Lightweight,
        &[dangerous_shell_call()]
    ));
}

#[test]
fn lightweight_mode_skips_safe_calls() {
    assert!(!should_review(
        AdvisorMode::Lightweight,
        &[safe_read_call()]
    ));
}

#[test]
fn lightweight_reviews_computer_use() {
    let call = ToolCall {
        id: "tc_3".into(),
        name: "computer.use".into(),
        arguments: r#"{"action":"screenshot"}"#.into(),
    };
    assert!(should_review(AdvisorMode::Lightweight, &[call]));
}

#[test]
fn lightweight_reviews_sensitive_file_write() {
    let call = ToolCall {
        id: "tc_4".into(),
        name: "fs.write".into(),
        arguments: r#"{"path":"/etc/passwd","content":"bad"}"#.into(),
    };
    assert!(should_review(AdvisorMode::Lightweight, &[call]));
}

#[test]
fn lightweight_skips_normal_file_write() {
    let call = ToolCall {
        id: "tc_5".into(),
        name: "fs.write".into(),
        arguments: r#"{"path":"src/main.rs","content":"fn main() {}"}"#.into(),
    };
    assert!(!should_review(AdvisorMode::Lightweight, &[call]));
}

// ── parse_advisor_response ──────────────────────────────────────────

#[test]
fn parse_approve_response() {
    let json = r#"{"verdict": "approve", "concerns": [], "summary": "All good."}"#;
    let review = parse_advisor_response(json);
    assert_eq!(review.verdict, AdvisorVerdict::Approve);
    assert!(review.concerns.is_empty());
    assert_eq!(review.summary, "All good.");
}

#[test]
fn parse_approve_with_warnings() {
    let json = r#"{
        "verdict": "approve_with_warnings",
        "concerns": [
            {"tool_name": "shell.exec", "severity": "medium", "message": "Watch out for side effects"}
        ],
        "summary": "Proceed with caution."
    }"#;
    let review = parse_advisor_response(json);
    assert_eq!(review.verdict, AdvisorVerdict::ApproveWithWarnings);
    assert_eq!(review.concerns.len(), 1);
    assert_eq!(review.concerns[0].tool_name, "shell.exec");
    assert_eq!(review.concerns[0].severity, "medium");
}

#[test]
fn parse_reject_response() {
    let json = r#"{
        "verdict": "reject",
        "concerns": [
            {"tool_name": "shell.exec", "severity": "high", "message": "rm -rf / is catastrophic"}
        ],
        "summary": "Blocked: destructive command."
    }"#;
    let review = parse_advisor_response(json);
    assert_eq!(review.verdict, AdvisorVerdict::Reject);
    assert_eq!(review.concerns.len(), 1);
    assert_eq!(review.concerns[0].severity, "high");
}

#[test]
fn parse_response_with_surrounding_text() {
    let text = r#"Here is my review:
    ```json
    {"verdict": "approve", "concerns": [], "summary": "Looks fine."}
    ```
    That's my analysis."#;
    let review = parse_advisor_response(text);
    assert_eq!(review.verdict, AdvisorVerdict::Approve);
    assert_eq!(review.summary, "Looks fine.");
}

#[test]
fn parse_malformed_response_defaults_to_approve() {
    let review = parse_advisor_response("I think this is fine but I can't format JSON");
    assert_eq!(review.verdict, AdvisorVerdict::Approve);
    assert!(review.concerns.is_empty());
    assert!(review.summary.contains("could not be parsed"));
}

#[test]
fn parse_empty_response_defaults_to_approve() {
    let review = parse_advisor_response("");
    assert_eq!(review.verdict, AdvisorVerdict::Approve);
}

#[test]
fn parse_unknown_verdict_defaults_to_approve() {
    let json = r#"{"verdict": "maybe", "concerns": [], "summary": "Unsure."}"#;
    let review = parse_advisor_response(json);
    assert_eq!(review.verdict, AdvisorVerdict::Approve);
}

#[test]
fn parse_concern_without_message_is_skipped() {
    let json = r#"{
        "verdict": "approve_with_warnings",
        "concerns": [
            {"tool_name": "shell.exec", "severity": "low"},
            {"tool_name": "fs.write", "severity": "medium", "message": "Valid concern"}
        ],
        "summary": "One valid concern."
    }"#;
    let review = parse_advisor_response(json);
    assert_eq!(review.concerns.len(), 1);
    assert_eq!(review.concerns[0].tool_name, "fs.write");
}

// ── is_high_risk_tool_call ──────────────────────────────────────────

#[test]
fn shell_with_sudo_is_high_risk() {
    let call = ToolCall {
        id: "tc_6".into(),
        name: "shell.exec".into(),
        arguments: r#"{"command":"sudo apt install foo"}"#.into(),
    };
    assert!(is_high_risk_tool_call(&call));
}

#[test]
fn shell_with_safe_command_is_not_high_risk() {
    let call = ToolCall {
        id: "tc_7".into(),
        name: "shell.exec".into(),
        arguments: r#"{"command":"cargo test"}"#.into(),
    };
    assert!(!is_high_risk_tool_call(&call));
}

#[test]
fn fs_write_to_env_is_high_risk() {
    let call = ToolCall {
        id: "tc_8".into(),
        name: "fs.write".into(),
        arguments: r#"{"path":".env","content":"SECRET=foo"}"#.into(),
    };
    assert!(is_high_risk_tool_call(&call));
}

#[test]
fn unknown_tool_is_not_high_risk() {
    let call = ToolCall {
        id: "tc_9".into(),
        name: "custom.tool".into(),
        arguments: "{}".into(),
    };
    assert!(!is_high_risk_tool_call(&call));
}

// ── review_tool_calls with FakeModelClient ──────────────────────────

#[tokio::test]
async fn review_with_fake_model_returns_parsed_review() {
    let response = r#"{"verdict": "approve_with_warnings", "concerns": [{"tool_name": "shell.exec", "severity": "high", "message": "Dangerous command"}], "summary": "Be careful."}"#;
    let model = agent_models::FakeModelClient::new(vec![response.to_string()]);
    let calls = vec![dangerous_shell_call()];

    let review = review_tool_calls(&model, "fake", &calls, "I will delete files", 5)
        .await
        .expect("review should succeed");

    assert_eq!(review.verdict, AdvisorVerdict::ApproveWithWarnings);
    assert_eq!(review.concerns.len(), 1);
    assert_eq!(review.advisor_profile, "fake");
    assert_eq!(review.summary, "Be careful.");
}

#[tokio::test]
async fn review_truncates_concerns_to_max() {
    let response = r#"{
        "verdict": "reject",
        "concerns": [
            {"tool_name": "a", "severity": "high", "message": "1"},
            {"tool_name": "b", "severity": "high", "message": "2"},
            {"tool_name": "c", "severity": "high", "message": "3"}
        ],
        "summary": "Many issues."
    }"#;
    let model = agent_models::FakeModelClient::new(vec![response.to_string()]);
    let calls = vec![dangerous_shell_call()];

    let review = review_tool_calls(&model, "fake", &calls, "", 2)
        .await
        .expect("review should succeed");

    assert_eq!(review.concerns.len(), 2);
}

// ── Helpers ─────────────────────────────────────────────────────────

fn dangerous_shell_call() -> ToolCall {
    ToolCall {
        id: "tc_1".into(),
        name: "shell.exec".into(),
        arguments: r#"{"command":"rm -rf /tmp/test"}"#.into(),
    }
}

fn safe_read_call() -> ToolCall {
    ToolCall {
        id: "tc_2".into(),
        name: "fs.read".into(),
        arguments: r#"{"path":"src/main.rs"}"#.into(),
    }
}

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

// --- Edge-case tests ---

#[test]
fn parse_review_malformed_json_defaults_approved() {
    // Missing closing brace — serde_json parse fails, should default to approved.
    let text = r#"{"approved": false, "findings": ["#;
    let (approved, findings) = ReviewerStrategy::parse_review(text);
    assert!(approved);
    assert!(findings.is_empty());
}

#[test]
fn parse_review_empty_string_defaults_approved() {
    let (approved, findings) = ReviewerStrategy::parse_review("");
    assert!(approved);
    assert!(findings.is_empty());
}

#[test]
fn parse_review_multiple_code_fences() {
    let text = "Here is the review:\n\n\
        ````json\n\
        ```json\n\
        {\"approved\": false, \"findings\": [{\"severity\": \"medium\", \"message\": \"Needs docs\"}]}\n\
        ```\n\
        ````";
    let (approved, findings) = ReviewerStrategy::parse_review(text);
    assert!(!approved);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, "medium");
    assert_eq!(findings[0].message, "Needs docs");
}

#[test]
fn parse_review_missing_severity_skips_finding() {
    // Finding without "severity" key — filter_map returns None, so it's skipped.
    let text = r#"{"approved": false, "findings": [{"message": "No severity here"}]}"#;
    let (approved, findings) = ReviewerStrategy::parse_review(text);
    assert!(!approved);
    assert!(findings.is_empty());
}

#[test]
fn parse_review_empty_severity_preserves_finding() {
    // Finding with empty string severity — valid, just empty.
    let text =
        r#"{"approved": false, "findings": [{"severity": "", "message": "Empty severity"}]}"#;
    let (approved, findings) = ReviewerStrategy::parse_review(text);
    assert!(!approved);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, "");
}

#[test]
fn parse_review_extra_unknown_fields_ignored() {
    let text = r#"{"approved": true, "confidence": 0.95, "findings": [{"severity": "low", "message": "Minor", "line": 42, "suggestion": "fix it"}]}"#;
    let (approved, findings) = ReviewerStrategy::parse_review(text);
    assert!(approved);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, "low");
    assert_eq!(findings[0].message, "Minor");
}

#[test]
fn review_diff_empty_string_no_findings() {
    let findings = ReviewerAgent::review_diff("");
    assert!(findings.is_empty());
}

#[test]
fn review_diff_additions_only_no_findings() {
    let diff = "+ use std::io;\n+ fn main() {}\n+ // new code\n";
    let findings = ReviewerAgent::review_diff(diff);
    assert!(findings.is_empty());
}

#[test]
fn reviewer_strategy_default_properties() {
    let strategy = ReviewerStrategy::default();
    assert_eq!(strategy.role(), AgentRole::Reviewer);
    assert!(strategy.model_profile_override().is_none());
    assert!(strategy.reasoning_effort_override().is_none());
    assert!(strategy.skills().is_empty());
    assert!(strategy.tools_allowlist().is_empty());
}

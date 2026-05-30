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

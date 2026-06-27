use super::parse_action;
use crate::browser::types::BrowserAction;

#[test]
fn parse_action_returns_typed_browser_action() {
    let action = parse_action(&serde_json::json!({
        "action": "navigate",
        "url": "https://example.com"
    }))
    .expect("navigate action should parse");

    match action {
        BrowserAction::Navigate { url } => assert_eq!(url, "https://example.com"),
        other => panic!("expected navigate action, got {other:?}"),
    }
}

#[test]
fn parse_action_returns_descriptive_error_for_invalid_payload() {
    let err = parse_action(&serde_json::json!({
        "action": "navigate"
    }))
    .expect_err("missing navigate url should fail");

    assert!(err.contains("Failed to parse browser action"));
    assert!(err.contains("url"));
}

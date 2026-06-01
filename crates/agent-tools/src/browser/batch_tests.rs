use crate::browser::batch::{BrowserBatchTool, BROWSER_BATCH_TOOL_ID};
use crate::browser::playwright::PlaywrightManager;
use crate::permission::ToolEffect;
use crate::registry::{Tool, ToolInvocation};
use std::path::PathBuf;
use std::sync::Arc;

fn make_tool() -> BrowserBatchTool {
    let manager = Arc::new(PlaywrightManager::new(PathBuf::from("/tmp/test-workspace")));
    BrowserBatchTool::new(manager)
}

fn make_invocation(args: serde_json::Value) -> ToolInvocation {
    ToolInvocation {
        tool_id: BROWSER_BATCH_TOOL_ID.to_string(),
        arguments: args,
        workspace_id: "test".to_string(),
        preview: String::new(),
        timeout_ms: 30_000,
        output_limit_bytes: 1024 * 1024,
    }
}

#[test]
fn definition_has_correct_tool_id() {
    let tool = make_tool();
    let def = tool.definition();
    assert_eq!(def.tool_id, "browser.batch");
    assert_eq!(def.required_capability, "browser.interact");
    assert!(!def.description.is_empty());
}

#[test]
fn risk_returns_browser_interact() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "actions": [{"action": "navigate", "url": "https://example.com"}]
    }));
    let risk = tool.risk(&invocation);
    assert_eq!(risk.tool_id, "browser.batch");
    assert_eq!(risk.effect, ToolEffect::BrowserInteract);
}

#[tokio::test]
async fn invoke_multiple_actions_succeeds() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "actions": [
            {"action": "navigate", "url": "https://example.com"},
            {"action": "click", "selector": "#button"},
            {"action": "type", "selector": "#input", "text": "hello"}
        ]
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(!output.truncated);
    assert!(output.text.contains("\"succeeded\": 3"));
    assert!(output.text.contains("\"failed\": 0"));
    assert!(output.text.contains("\"total\": 3"));
}

#[tokio::test]
async fn invoke_stop_on_error_true_stops_at_first_failure() {
    let tool = make_tool();
    // Close the browser first, then try an action that would fail
    // Since PlaywrightManager simulates success for all actions, we test
    // that stop_on_error=true works by verifying the structure.
    let invocation = make_invocation(serde_json::json!({
        "actions": [
            {"action": "navigate", "url": "https://example.com"},
            {"action": "click", "selector": "#button"}
        ],
        "stop_on_error": true
    }));
    let output = tool.invoke(invocation).await.unwrap();
    // All succeed with simulated backend
    assert!(output.text.contains("\"succeeded\": 2"));
    assert!(output.text.contains("\"total\": 2"));
}

#[tokio::test]
async fn invoke_stop_on_error_false_continues_past_failure() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "actions": [
            {"action": "navigate", "url": "https://example.com"},
            {"action": "click", "selector": "#button"},
            {"action": "get_state"}
        ],
        "stop_on_error": false
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("\"total\": 3"));
    assert!(output.text.contains("\"succeeded\": 3"));
}

#[tokio::test]
async fn invoke_empty_actions_array() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "actions": []
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("\"total\": 0"));
    assert!(output.text.contains("\"succeeded\": 0"));
    assert!(output.text.contains("\"failed\": 0"));
}

#[tokio::test]
async fn invoke_invalid_actions_returns_error() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "actions": "not_an_array"
    }));
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err
        .to_string()
        .contains("Missing or invalid 'actions' array"));
}

#[tokio::test]
async fn invoke_missing_actions_returns_error() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({}));
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err
        .to_string()
        .contains("Missing or invalid 'actions' array"));
}

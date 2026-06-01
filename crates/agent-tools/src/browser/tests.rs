use crate::browser::tool::BROWSER_TOOL_ID;
use crate::browser::BrowserTool;
use crate::permission::ToolEffect;
use crate::registry::{Tool, ToolInvocation};
use std::path::PathBuf;

fn make_tool() -> BrowserTool {
    BrowserTool::new(PathBuf::from("/tmp/test-workspace"))
}

fn make_invocation(args: serde_json::Value) -> ToolInvocation {
    ToolInvocation {
        tool_id: BROWSER_TOOL_ID.to_string(),
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
    assert_eq!(def.tool_id, "browser.action");
    assert_eq!(def.required_capability, "browser.interact");
    assert!(!def.description.is_empty());
}

#[test]
fn risk_returns_browser_interact() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "navigate",
        "url": "https://example.com"
    }));
    let risk = tool.risk(&invocation);
    assert_eq!(risk.tool_id, "browser.action");
    assert_eq!(risk.effect, ToolEffect::BrowserInteract);
}

#[tokio::test]
async fn invoke_navigate() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "navigate",
        "url": "https://example.com"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(!output.truncated);
    assert!(output.text.contains("https://example.com"));
    assert!(output.text.contains("\"success\": true"));
}

#[tokio::test]
async fn invoke_screenshot() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "screenshot",
        "full_page": true
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Screenshot captured"));
    assert!(output.text.contains("base64-placeholder"));
}

#[tokio::test]
async fn invoke_invalid_action_returns_error() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "nonexistent_action"
    }));
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Invalid browser action"));
}

#[tokio::test]
async fn invoke_close() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "close"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Browser closed"));
    assert!(output.text.contains("\"success\": true"));
}

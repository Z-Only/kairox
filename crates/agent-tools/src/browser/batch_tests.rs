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
        session_id: "ses_test".into(),
        preview: String::new(),
        timeout_ms: 30_000,
        output_limit_bytes: 1024 * 1024,
    }
}

/// Check whether Node.js + Playwright are available for integration tests.
fn playwright_available() -> bool {
    std::process::Command::new("node")
        .arg("-e")
        .arg("try { require('playwright'); } catch { process.exit(1); }")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// --- Unit tests (no Node.js required) ---

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
async fn invoke_empty_actions_array() {
    // Empty array doesn't trigger the bridge, so works without Playwright
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

// --- Integration tests (require Node.js + Playwright) ---

#[tokio::test]
async fn invoke_multiple_actions_real() {
    if !playwright_available() {
        eprintln!("Skipping: Playwright not available");
        return;
    }
    let manager = Arc::new(PlaywrightManager::new(std::env::temp_dir()));
    let tool = BrowserBatchTool::new(manager.clone());
    let invocation = make_invocation(serde_json::json!({
        "actions": [
            {"action": "navigate", "url": "https://example.com"},
            {"action": "get_state"},
            {"action": "screenshot"}
        ]
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(!output.truncated);
    assert!(output.text.contains("\"succeeded\": 3"));
    assert!(output.text.contains("\"failed\": 0"));
    assert!(output.text.contains("\"total\": 3"));

    manager.shutdown().await;
}

#[tokio::test]
async fn invoke_batch_stop_on_error_with_bad_selector() {
    if !playwright_available() {
        eprintln!("Skipping: Playwright not available");
        return;
    }
    let manager = Arc::new(PlaywrightManager::new(std::env::temp_dir()));
    let tool = BrowserBatchTool::new(manager.clone());
    // Navigate, then click a nonexistent selector (will fail with timeout)
    let invocation = make_invocation(serde_json::json!({
        "actions": [
            {"action": "navigate", "url": "https://example.com"},
            {"action": "click", "selector": "#definitely-does-not-exist-xyz"},
            {"action": "get_state"}
        ],
        "stop_on_error": true
    }));
    let output = tool.invoke(invocation).await.unwrap();
    // First action succeeds, second fails, third is skipped
    assert!(output.text.contains("\"succeeded\": 1"));
    assert!(output.text.contains("\"total\": 2")); // stopped at index 1

    manager.shutdown().await;
}

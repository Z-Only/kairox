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

#[test]
fn definition_schema_has_all_action_variants() {
    let tool = make_tool();
    let def = tool.definition();
    let schema = &def.parameters;
    let actions = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("action enum should be an array");
    let action_strs: Vec<&str> = actions.iter().filter_map(|v| v.as_str()).collect();
    for expected in [
        "navigate",
        "click",
        "type",
        "scroll",
        "hover",
        "screenshot",
        "get_text",
        "wait",
        "form_fill",
        "get_state",
        "close",
    ] {
        assert!(
            action_strs.contains(&expected),
            "Missing action variant: {}",
            expected
        );
    }
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

#[test]
fn playwright_preflight_error_explains_missing_dependency() {
    let message = crate::browser::playwright::playwright_preflight_error(
        "/repo",
        "Cannot find module 'playwright'",
    );

    assert!(message.contains("Playwright dependency preflight failed"));
    assert!(message.contains("/repo"));
    assert!(message.contains("Cannot find module 'playwright'"));
    assert!(message.contains("NODE_PATH"));
    assert!(message.contains("playwright install chromium"));
}

#[test]
fn playwright_process_output_detail_prefers_stderr() {
    let output = std::process::Output {
        status: std::process::Command::new("sh")
            .arg("-c")
            .arg("exit 7")
            .status()
            .expect("status should be available"),
        stdout: b"stdout detail\n".to_vec(),
        stderr: b"stderr detail\n".to_vec(),
    };

    let detail = crate::browser::playwright::process_output_detail(&output);
    assert_eq!(detail, "stderr detail");
}

#[test]
fn playwright_process_output_detail_uses_stdout_when_stderr_empty() {
    let output = std::process::Output {
        status: std::process::Command::new("sh")
            .arg("-c")
            .arg("exit 7")
            .status()
            .expect("status should be available"),
        stdout: b"stdout detail\n".to_vec(),
        stderr: Vec::new(),
    };

    let detail = crate::browser::playwright::process_output_detail(&output);
    assert_eq!(detail, "stdout detail");
}

#[test]
fn playwright_node_path_env_prepends_node_modules() {
    let value = crate::browser::playwright::node_path_env_with_existing(
        std::path::Path::new("/repo/node_modules"),
        Some(std::ffi::OsString::from("/existing/node_modules")),
    );

    let value = value.to_string_lossy();
    assert!(value.starts_with("/repo/node_modules"));
    assert!(value.contains("/existing/node_modules"));
}

fn fake_playwright_install(root: &std::path::Path) -> std::path::PathBuf {
    let node_modules = root.join("node_modules");
    let package_dir = node_modules.join("playwright");
    std::fs::create_dir_all(&package_dir).expect("playwright package dir should be created");
    std::fs::write(
        package_dir.join("package.json"),
        r#"{"name":"playwright","version":"0.0.0","main":"index.js"}"#,
    )
    .expect("package manifest should be written");
    std::fs::write(package_dir.join("index.js"), "module.exports = {};")
        .expect("package entrypoint should be written");
    node_modules
}

#[tokio::test]
async fn playwright_resolution_uses_node_path_fallback() {
    let node_path = std::path::Path::new("node");
    if !std::process::Command::new(node_path)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        eprintln!("Skipping: Node.js not available");
        return;
    }

    let repo_root = tempfile::tempdir().expect("repo root should be created");
    let node_modules = fake_playwright_install(repo_root.path());

    let isolated_workspace = tempfile::tempdir().expect("isolated workspace should be created");
    assert!(crate::browser::playwright::playwright_resolves(
        node_path,
        isolated_workspace.path(),
        None
    )
    .await
    .is_err());
    assert!(crate::browser::playwright::playwright_resolves(
        node_path,
        isolated_workspace.path(),
        Some(&node_modules)
    )
    .await
    .is_ok());

    let resolved =
        crate::browser::playwright::resolve_playwright_node_modules(node_path, repo_root.path())
            .await
            .expect("repo Playwright install should resolve");
    assert_eq!(
        std::fs::canonicalize(resolved).expect("resolved path should canonicalize"),
        std::fs::canonicalize(node_modules).expect("expected path should canonicalize")
    );
}

#[tokio::test]
async fn playwright_preflight_accepts_workspace_playwright_install() {
    let node_path = std::path::Path::new("node");
    if !std::process::Command::new(node_path)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        eprintln!("Skipping: Node.js not available");
        return;
    }

    let workspace = tempfile::tempdir().expect("workspace should be created");
    fake_playwright_install(workspace.path());

    let node_path_override =
        crate::browser::playwright::preflight_playwright_dependencies(node_path, workspace.path())
            .await
            .expect("workspace Playwright install should pass preflight");
    assert!(node_path_override.is_none());
}

#[tokio::test]
async fn playwright_preflight_uses_repository_fallback() {
    let node_path = std::path::Path::new("node");
    if !playwright_available() {
        eprintln!("Skipping: repository Playwright not available");
        return;
    }

    let isolated_workspace = tempfile::tempdir().expect("isolated workspace should be created");
    let node_path_override = crate::browser::playwright::preflight_playwright_dependencies(
        node_path,
        isolated_workspace.path(),
    )
    .await
    .expect("repository fallback should pass preflight");
    assert!(node_path_override.is_some());
}

// --- Integration tests (require Node.js + Playwright) ---

#[tokio::test]
async fn invoke_navigate_real() {
    if !playwright_available() {
        eprintln!("Skipping: Playwright not available");
        return;
    }
    let tool = BrowserTool::new(std::env::temp_dir());
    let invocation = make_invocation(serde_json::json!({
        "action": "navigate",
        "url": "https://example.com"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(!output.truncated);
    assert!(output.text.contains("example.com"));
    assert!(output.text.contains("success: true"));

    // Cleanup
    tool.manager().shutdown().await;
}

#[tokio::test]
async fn invoke_screenshot_returns_data_uri() {
    if !playwright_available() {
        eprintln!("Skipping: Playwright not available");
        return;
    }
    let tool = BrowserTool::new(std::env::temp_dir());
    // Navigate first so there's content
    let _ = tool
        .invoke(make_invocation(serde_json::json!({
            "action": "navigate",
            "url": "https://example.com"
        })))
        .await;

    let invocation = make_invocation(serde_json::json!({
        "action": "screenshot"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("success: true"));
    // Screenshot should be embedded as a markdown data URI.
    assert!(
        output.text.contains("![screenshot](data:image/png;base64,"),
        "Screenshot should be embedded as a markdown data URI"
    );

    tool.manager().shutdown().await;
}

#[tokio::test]
async fn invoke_get_state_and_close() {
    if !playwright_available() {
        eprintln!("Skipping: Playwright not available");
        return;
    }
    let tool = BrowserTool::new(std::env::temp_dir());
    let state_output = tool
        .invoke(make_invocation(serde_json::json!({
            "action": "get_state"
        })))
        .await
        .unwrap();
    assert!(state_output.text.contains("success: true"));

    let close_output = tool
        .invoke(make_invocation(serde_json::json!({
            "action": "close"
        })))
        .await
        .unwrap();
    assert!(close_output.text.contains("Browser closed"));
}

#[tokio::test]
async fn graceful_error_without_playwright() {
    // Test that the manager produces a clear error when Node.js is present
    // but the bridge script encounters a missing playwright module.
    // This test is conceptual — it verifies the error path exists.
    // A full test would require mocking the node environment.
    let manager =
        crate::browser::playwright::PlaywrightManager::new(PathBuf::from("/nonexistent/workspace"));
    // If node is available, it will try to start and may fail gracefully.
    // If node is not available, ensure_running returns a clear error.
    if let Err(err) = manager.ensure_running().await {
        // Error should mention Node.js or Playwright, not panic
        assert!(
            err.contains("Node")
                || err.contains("node")
                || err.contains("Playwright")
                || err.contains("playwright")
                || err.contains("temp dir")
                || err.contains("Failed"),
            "Error should be descriptive: {}",
            err
        );
    }
    manager.shutdown().await;
}

// --- format_browser_result tests ---

#[test]
fn format_browser_result_without_screenshot() {
    use super::tool::format_browser_result;
    use super::types::BrowserResult;

    let result = BrowserResult {
        success: true,
        output: "Navigated to https://example.com".into(),
        screenshot: None,
        current_url: Some("https://example.com".into()),
        title: Some("Example Domain".into()),
    };
    let text = format_browser_result(&result);
    assert!(text.contains("success: true"));
    assert!(text.contains("output: Navigated to https://example.com"));
    assert!(text.contains("current_url: https://example.com"));
    assert!(text.contains("title: Example Domain"));
    assert!(!text.contains("data:image/png;base64"));
}

#[test]
fn format_browser_result_with_screenshot_embeds_data_uri() {
    use super::tool::format_browser_result;
    use super::types::BrowserResult;

    let result = BrowserResult {
        success: true,
        output: "Screenshot captured".into(),
        screenshot: Some("iVBORw0KGgo=".into()),
        current_url: None,
        title: None,
    };
    let text = format_browser_result(&result);
    assert!(text.contains("success: true"));
    assert!(text.contains("![screenshot](data:image/png;base64,iVBORw0KGgo=)"));
}

use super::*;
use serde_json::json;

fn make_invocation(
    pattern: &str,
    path: Option<&str>,
    file_glob: Option<&str>,
    max_results: usize,
) -> ToolInvocation {
    let mut args = json!({
        "pattern": pattern,
        "max_results": max_results,
    });
    if let Some(p) = path {
        args["path"] = json!(p);
    }
    if let Some(g) = file_glob {
        args["file_glob"] = json!(g);
    }
    ToolInvocation {
        tool_id: shell::SEARCH_TOOL_ID.to_string(),
        arguments: args,
        workspace_id: "test".to_string(),
        preview: format!("search {}", pattern),
        timeout_ms: 10000,
        output_limit_bytes: 102_400,
    }
}

// ── Full Tool::invoke tests ──────────────────────────────────────────

#[tokio::test]
async fn search_tool_invocation_works_with_fallback() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(dir.path().join("hello.txt"), "world says hello\n")
        .await
        .unwrap();

    let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
    let invocation = make_invocation("hello", None, None, 50);
    let output = tool.invoke(invocation).await.unwrap();

    assert!(!output.text.is_empty());
    assert!(output.text.contains("hello.txt"));
}

#[tokio::test]
async fn search_tool_empty_pattern_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
    let invocation = make_invocation("", None, None, 50);
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ToolError::ExecutionFailed(msg) => assert_eq!(msg, "empty search pattern"),
        other => panic!("expected ExecutionFailed, got {:?}", other),
    }
}

// ── Tool trait tests ──────────────────────────────────────────────────

#[test]
fn definition_returns_correct_id() {
    let dir = tempfile::tempdir().unwrap();
    let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
    let def = tool.definition();
    assert_eq!(def.tool_id, shell::SEARCH_TOOL_ID);
    assert_eq!(def.required_capability, "search.ripgrep");
}

#[test]
fn risk_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let tool = RipgrepSearchTool::new(dir.path().to_path_buf());
    let inv = make_invocation("test", None, None, 10);
    let risk = tool.risk(&inv);
    assert_eq!(risk, ToolRisk::read(shell::SEARCH_TOOL_ID));
}

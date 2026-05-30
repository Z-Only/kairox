use super::*;
use crate::registry::ToolInvocation;
use std::io::Write as IoWrite;

fn temp_workspace() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn definition_has_correct_tool_id() {
    let dir = temp_workspace();
    let tool = FsReadTool::new(dir.path().to_path_buf());
    let def = tool.definition();
    assert_eq!(def.tool_id, "fs.read");
    assert_eq!(def.required_capability, "filesystem.read");
}

#[tokio::test]
async fn read_file_within_workspace() {
    let dir = temp_workspace();
    let file_path = dir.path().join("hello.txt");
    let mut f = std::fs::File::create(&file_path).unwrap();
    f.write_all(b"Hello, world!").unwrap();

    let tool = FsReadTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.read".into(),
        arguments: serde_json::json!({"path": "hello.txt"}),
        workspace_id: "wrk_test".into(),
        preview: "fs.read(hello.txt)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let output = tool.invoke(invocation).await.unwrap();
    assert_eq!(output.text, "Hello, world!");
    assert!(!output.truncated);
}

#[tokio::test]
async fn read_file_truncates_at_output_limit() {
    let dir = temp_workspace();
    let file_path = dir.path().join("large.txt");
    let mut f = std::fs::File::create(&file_path).unwrap();
    let large_content = "x".repeat(1000);
    f.write_all(large_content.as_bytes()).unwrap();

    let tool = FsReadTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.read".into(),
        arguments: serde_json::json!({"path": "large.txt"}),
        workspace_id: "wrk_test".into(),
        preview: "fs.read(large.txt)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 100,
    };
    let output = tool.invoke(invocation).await.unwrap();
    assert_eq!(output.text.len(), 100);
    assert!(output.truncated);
}

#[tokio::test]
async fn read_file_outside_workspace_returns_escape_error() {
    let dir = temp_workspace();
    let outside_file = dir.path().join("outside.txt");
    std::fs::write(&outside_file, "secret").unwrap();

    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let tool = FsReadTool::new(workspace);

    let invocation = ToolInvocation {
        tool_id: "fs.read".into(),
        arguments: serde_json::json!({"path": "../outside.txt"}),
        workspace_id: "wrk_test".into(),
        preview: "fs.read(../outside.txt)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("escape") || err.contains("WorkspaceEscape"),
        "Expected workspace escape error, got: {err}"
    );
}

#[tokio::test]
async fn read_nonexistent_file_returns_error() {
    let dir = temp_workspace();
    let tool = FsReadTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.read".into(),
        arguments: serde_json::json!({"path": "does_not_exist.txt"}),
        workspace_id: "wrk_test".into(),
        preview: "fs.read(does_not_exist.txt)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
}

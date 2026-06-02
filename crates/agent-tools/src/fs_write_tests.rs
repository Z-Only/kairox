use super::*;
use crate::registry::ToolInvocation;

fn temp_workspace() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn make_invocation(tool_id: &str, args: serde_json::Value) -> ToolInvocation {
    ToolInvocation {
        tool_id: tool_id.into(),
        arguments: args,
        workspace_id: "wrk_test".into(),
        session_id: "ses_test".into(),
        preview: format!("{tool_id}()"),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    }
}

#[test]
fn write_tool_definition() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());
    let def = tool.definition();
    assert_eq!(def.tool_id, "fs.write");
    assert_eq!(def.required_capability, "filesystem.write");
}

#[tokio::test]
async fn write_new_file() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());

    let invocation = make_invocation(
        "fs.write",
        serde_json::json!({
            "path": "new.txt",
            "content": "hello there"
        }),
    );
    let output = tool.invoke(invocation).await.unwrap();
    assert_eq!(output.text, "Written 11 bytes to new.txt");
    assert!(!output.truncated);

    let content = std::fs::read_to_string(dir.path().join("new.txt")).unwrap();
    assert_eq!(content, "hello there");
}

#[tokio::test]
async fn write_file_creates_parent_dirs_by_default() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());

    let invocation = make_invocation(
        "fs.write",
        serde_json::json!({
            "path": "a/b/c/deep.txt",
            "content": "nested"
        }),
    );
    let output = tool.invoke(invocation).await.unwrap();
    assert_eq!(output.text, "Written 6 bytes to a/b/c/deep.txt");

    let content = std::fs::read_to_string(dir.path().join("a/b/c/deep.txt")).unwrap();
    assert_eq!(content, "nested");
}

#[tokio::test]
async fn write_file_no_create_dirs_when_false() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());

    let invocation = make_invocation(
        "fs.write",
        serde_json::json!({
            "path": "no/such/dir/file.txt",
            "content": "fail",
            "create_dirs": false
        }),
    );
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn write_file_backs_up_existing() {
    let dir = temp_workspace();
    std::fs::write(dir.path().join("data.txt"), "original").unwrap();

    let tool = FsWriteTool::new(dir.path().to_path_buf());
    let invocation = make_invocation(
        "fs.write",
        serde_json::json!({
            "path": "data.txt",
            "content": "updated"
        }),
    );
    let output = tool.invoke(invocation).await.unwrap();
    assert_eq!(output.text, "Written 7 bytes to data.txt");

    // New content
    assert_eq!(
        std::fs::read_to_string(dir.path().join("data.txt")).unwrap(),
        "updated"
    );
    // Backup
    assert_eq!(
        std::fs::read_to_string(dir.path().join("data.txt.bak")).unwrap(),
        "original"
    );
}

#[tokio::test]
async fn write_file_rejects_escape() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());

    let invocation = make_invocation(
        "fs.write",
        serde_json::json!({
            "path": "../escape.txt",
            "content": "nope"
        }),
    );
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("escape") || msg.contains("WorkspaceEscape"),
        "Expected escape error, got: {msg}"
    );
}

#[tokio::test]
async fn write_file_missing_content_returns_error() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());

    let invocation = make_invocation(
        "fs.write",
        serde_json::json!({
            "path": "file.txt"
        }),
    );
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("content"),
        "Expected missing content error, got: {msg}"
    );
}

#[tokio::test]
async fn write_overwrites_previous_backup() {
    let dir = temp_workspace();
    // First version
    std::fs::write(dir.path().join("log.txt"), "v1").unwrap();
    // Write v2 → backup is "v1"
    let tool = FsWriteTool::new(dir.path().to_path_buf());
    let invocation = make_invocation(
        "fs.write",
        serde_json::json!({ "path": "log.txt", "content": "v2" }),
    );
    tool.invoke(invocation).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("log.txt.bak")).unwrap(),
        "v1"
    );

    // Write v3 → backup is now "v2"
    let invocation = make_invocation(
        "fs.write",
        serde_json::json!({ "path": "log.txt", "content": "v3" }),
    );
    tool.invoke(invocation).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("log.txt.bak")).unwrap(),
        "v2"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("log.txt")).unwrap(),
        "v3"
    );
}

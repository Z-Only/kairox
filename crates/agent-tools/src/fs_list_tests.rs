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
fn list_tool_definition() {
    let dir = temp_workspace();
    let tool = FsListTool::new(dir.path().to_path_buf());
    let def = tool.definition();
    assert_eq!(def.tool_id, "fs.list");
    assert_eq!(def.required_capability, "filesystem.read");
}

#[tokio::test]
async fn list_empty_directory() {
    let dir = temp_workspace();
    let tool = FsListTool::new(dir.path().to_path_buf());

    let invocation = make_invocation("fs.list", serde_json::json!({ "path": "." }));
    let output = tool.invoke(invocation).await.unwrap();
    let entries: Vec<FsListEntry> = serde_json::from_str(&output.text).unwrap();
    assert!(entries.is_empty());
}

#[tokio::test]
async fn list_directory_with_files_and_dirs() {
    let dir = temp_workspace();
    std::fs::write(dir.path().join("alpha.txt"), "a").unwrap();
    std::fs::write(dir.path().join("beta.txt"), "bb").unwrap();
    std::fs::create_dir(dir.path().join("subdir")).unwrap();

    let tool = FsListTool::new(dir.path().to_path_buf());
    let invocation = make_invocation("fs.list", serde_json::json!({ "path": "." }));
    let output = tool.invoke(invocation).await.unwrap();
    let entries: Vec<FsListEntry> = serde_json::from_str(&output.text).unwrap();

    // Directories first, then alphabetical
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].entry_type, "dir");
    assert_eq!(entries[0].name, "subdir");
    assert_eq!(entries[1].entry_type, "file");
    assert_eq!(entries[1].name, "alpha.txt");
    assert_eq!(entries[2].entry_type, "file");
    assert_eq!(entries[2].name, "beta.txt");

    // Check size
    assert_eq!(entries[1].size_bytes, 1); // "a"
    assert_eq!(entries[2].size_bytes, 2); // "bb"
}

#[tokio::test]
async fn list_recursive() {
    let dir = temp_workspace();
    std::fs::write(dir.path().join("root.txt"), "r").unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main(){}").unwrap();
    std::fs::create_dir(dir.path().join("src/utils")).unwrap();
    std::fs::write(dir.path().join("src/utils/mod.rs"), "").unwrap();

    let tool = FsListTool::new(dir.path().to_path_buf());
    let invocation = make_invocation(
        "fs.list",
        serde_json::json!({ "path": ".", "recursive": true }),
    );
    let output = tool.invoke(invocation).await.unwrap();
    let entries: Vec<FsListEntry> = serde_json::from_str(&output.text).unwrap();

    // All entries should have paths relative to workspace root
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"root.txt"));
    assert!(names.contains(&"src"));
    assert!(names.contains(&"src/main.rs"));
    assert!(names.contains(&"src/utils"));
    assert!(names.contains(&"src/utils/mod.rs"));
}

#[tokio::test]
async fn list_non_directory_returns_error() {
    let dir = temp_workspace();
    std::fs::write(dir.path().join("file.txt"), "data").unwrap();

    let tool = FsListTool::new(dir.path().to_path_buf());
    let invocation = make_invocation("fs.list", serde_json::json!({ "path": "file.txt" }));
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not a directory"),
        "Expected not-a-directory error, got: {msg}"
    );
}

#[tokio::test]
async fn list_escape_returns_error() {
    let dir = temp_workspace();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();

    let tool = FsListTool::new(workspace);
    let invocation = make_invocation("fs.list", serde_json::json!({ "path": ".." }));
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list_defaults_to_current_directory() {
    let dir = temp_workspace();
    std::fs::write(dir.path().join("hello.txt"), "hi").unwrap();

    let tool = FsListTool::new(dir.path().to_path_buf());
    // No "path" argument — should default to "."
    let invocation = make_invocation("fs.list", serde_json::json!({}));
    let output = tool.invoke(invocation).await.unwrap();
    let entries: Vec<FsListEntry> = serde_json::from_str(&output.text).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "hello.txt");
}

#[tokio::test]
async fn list_subdirectory() {
    let dir = temp_workspace();
    std::fs::create_dir(dir.path().join("inner")).unwrap();
    std::fs::write(dir.path().join("inner/file.txt"), "content").unwrap();

    let tool = FsListTool::new(dir.path().to_path_buf());
    let invocation = make_invocation("fs.list", serde_json::json!({ "path": "inner" }));
    let output = tool.invoke(invocation).await.unwrap();
    let entries: Vec<FsListEntry> = serde_json::from_str(&output.text).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "inner/file.txt");
}

#[tokio::test]
async fn list_entry_has_modified_timestamp() {
    let dir = temp_workspace();
    std::fs::write(dir.path().join("timed.txt"), "t").unwrap();

    let tool = FsListTool::new(dir.path().to_path_buf());
    let invocation = make_invocation("fs.list", serde_json::json!({}));
    let output = tool.invoke(invocation).await.unwrap();
    let entries: Vec<FsListEntry> = serde_json::from_str(&output.text).unwrap();
    assert!(!entries[0].modified.is_empty());
    // Should be a valid Unix timestamp (numeric string)
    assert!(
        entries[0].modified.parse::<u64>().is_ok(),
        "Modified should be a Unix timestamp, got: {}",
        entries[0].modified
    );
}

use crate::fs_helpers::resolve_workspace_write_path;
use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FsWriteTool {
    workspace_root: PathBuf,
}

impl FsWriteTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl Tool for FsWriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "fs.write".into(),
            description: "Write content to a file within the workspace (atomic, with backup)"
                .into(),
            required_capability: "filesystem.write".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file within the workspace"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    },
                    "create_dirs": {
                        "type": "boolean",
                        "description": "Whether to create parent directories if they don't exist (default: true)"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk {
        let _ = invocation;
        ToolRisk::write("fs.write")
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let relative_path = invocation.arguments["path"].as_str().unwrap_or("");
        let content = invocation
            .arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::ToolError::ExecutionFailed("missing required argument: content".into())
            })?;
        let create_dirs = invocation
            .arguments
            .get("create_dirs")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let path = resolve_workspace_write_path(&self.workspace_root, relative_path)?;

        // Create parent directories if needed
        if create_dirs {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        // Backup existing file
        if path.exists() {
            let backup = {
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                path.with_file_name(format!("{file_name}.bak"))
            };
            tokio::fs::copy(&path, &backup).await?;
        }

        // Atomic write: write to temp file then rename
        let pid = std::process::id();
        let tmp_path = {
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            path.with_file_name(format!("{file_name}.tmp.{pid}"))
        };

        tokio::fs::write(&tmp_path, content.as_bytes()).await?;
        tokio::fs::rename(&tmp_path, &path).await?;

        Ok(ToolOutput {
            text: format!("Written {} bytes to {}", content.len(), relative_path),
            truncated: false,
        })
    }
}

#[cfg(test)]
mod tests {
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
}

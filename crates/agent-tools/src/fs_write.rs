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
#[path = "fs_write_tests.rs"]
mod tests;

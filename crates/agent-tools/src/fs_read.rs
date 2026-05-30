use crate::fs_helpers::resolve_workspace_read_path;
use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FsReadTool {
    workspace_root: PathBuf,
}

impl FsReadTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl Tool for FsReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "fs.read".into(),
            description: "Read a UTF-8 file within the workspace".into(),
            required_capability: "filesystem.read".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file within the workspace"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk {
        let _ = invocation;
        ToolRisk::read("fs.read")
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let relative_path = invocation.arguments["path"].as_str().unwrap_or("");
        let path = resolve_workspace_read_path(&self.workspace_root, relative_path)?;
        let mut text = tokio::fs::read_to_string(Path::new(&path)).await?;
        let truncated = text.len() > invocation.output_limit_bytes;
        if truncated {
            text.truncate(invocation.output_limit_bytes);
        }
        Ok(ToolOutput { text, truncated })
    }
}

#[cfg(test)]
#[path = "fs_read_tests.rs"]
mod tests;

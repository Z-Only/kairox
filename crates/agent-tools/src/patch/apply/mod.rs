mod executor;
mod hunk;
mod path;

use crate::patch::parse::parse_unified_diff;
use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use crate::shell::PATCH_TOOL_ID;
use crate::ToolError;
use async_trait::async_trait;
use std::path::PathBuf;

const UNIFIED_DIFF_FORMAT_HINT: &str =
    "Expected unified diff with file headers like --- a/path and +++ b/path, \
and hunk headers like @@ -old_start,old_count +new_start,new_count @@.";
const CODEX_APPLY_PATCH_HINT: &str =
    "Codex apply_patch format (*** Begin Patch / *** Update File) is not supported by patch.apply.";

pub struct PatchApplyTool {
    workspace_root: PathBuf,
}

impl PatchApplyTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl Tool for PatchApplyTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: PATCH_TOOL_ID.to_string(),
            description: format!(
                "Apply a unified diff patch to workspace files. {} Do not send Codex *** Begin Patch format.",
                UNIFIED_DIFF_FORMAT_HINT
            ),
            required_capability: "patch.apply".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": format!(
                            "Unified diff patch text to apply. {}",
                            UNIFIED_DIFF_FORMAT_HINT
                        )
                    }
                },
                "required": ["patch"]
            }),
        }
    }

    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk {
        let patch_text = invocation
            .arguments
            .get("patch")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match parse_unified_diff(patch_text) {
            Ok(file_patches) => {
                let has_new_or_delete =
                    file_patches.iter().any(|fp| fp.is_new_file || fp.is_delete);
                if has_new_or_delete {
                    ToolRisk::destructive(PATCH_TOOL_ID)
                } else {
                    ToolRisk::write(PATCH_TOOL_ID)
                }
            }
            Err(_) => {
                // If we can't parse, assume write (least surprising)
                ToolRisk::write(PATCH_TOOL_ID)
            }
        }
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let patch_text = invocation
            .arguments
            .get("patch")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Parse the diff
        let file_patches =
            parse_unified_diff(patch_text).map_err(|e| parse_failed_error(patch_text, e))?;

        if file_patches.is_empty() {
            return Err(ToolError::PatchParseFailed(
                "no file patches found in diff".to_string(),
            ));
        }

        // Resolve paths
        let resolved = executor::resolve_patches(&self.workspace_root, &file_patches)?;

        let _locks = executor::acquire_file_locks(&resolved).await;

        // Build the final file states before writing so failures stay all-or-nothing.
        let plan = executor::plan_patches(&resolved).await?;
        let affected_files = executor::apply_patches(plan).await?;

        Ok(ToolOutput {
            text: format!(
                "Applied patch to {} file(s): {}",
                affected_files.len(),
                affected_files.join(", ")
            ),
            truncated: false,
            exit_code: None,
            images: vec![],
        })
    }
}

fn parse_failed_error(patch_text: &str, err: crate::patch::parse::PatchParseError) -> ToolError {
    let mut message = err.to_string();
    if looks_like_codex_apply_patch(patch_text) {
        message.push_str("\n\n");
        message.push_str(CODEX_APPLY_PATCH_HINT);
        message.push(' ');
        message.push_str(UNIFIED_DIFF_FORMAT_HINT);
    }
    ToolError::PatchParseFailed(message)
}

fn looks_like_codex_apply_patch(patch_text: &str) -> bool {
    patch_text.contains("*** Begin Patch") || patch_text.contains("*** Update File:")
}

#[cfg(test)]
mod tests;

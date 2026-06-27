mod executor;
mod hunk;
mod path;

use crate::patch::parse::{parse_unified_diff, FilePatch, Hunk, PatchLine};
use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use crate::shell::PATCH_TOOL_ID;
use crate::ToolError;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

const UNIFIED_DIFF_FORMAT_HINT: &str =
    "Expected unified diff with file headers like --- a/path and +++ b/path, \
and hunk headers like @@ -old_start,old_count +new_start,new_count @@.";
const CODEX_APPLY_PATCH_HINT: &str =
    "Codex apply_patch compatibility supports simple *** Update File hunks with context.";

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
                "Apply a unified diff patch, or a simple Codex *** Begin Patch / *** Update File patch, to workspace files. {}",
                UNIFIED_DIFF_FORMAT_HINT
            ),
            required_capability: "patch.apply".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": format!(
                            "Unified diff patch text to apply. Simple Codex *** Begin Patch / *** Update File patches are accepted. {}",
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
            Err(_)
                if patch_text.contains("*** Add File:")
                    || patch_text.contains("*** Delete File:") =>
            {
                ToolRisk::destructive(PATCH_TOOL_ID)
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
        let file_patches = parse_patch_text(&self.workspace_root, patch_text)?;

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

fn parse_patch_text(workspace_root: &Path, patch_text: &str) -> crate::Result<Vec<FilePatch>> {
    match parse_unified_diff(patch_text) {
        Ok(file_patches) => Ok(file_patches),
        Err(err)
            if patch_text.contains("*** Begin Patch")
                || patch_text.contains("*** Update File:") =>
        {
            parse_codex_update_patch(workspace_root, patch_text).map_err(|compat_err| {
                ToolError::PatchParseFailed(format!(
                    "{}\n\n{} {}",
                    err, CODEX_APPLY_PATCH_HINT, compat_err
                ))
            })
        }
        Err(err) => Err(ToolError::PatchParseFailed(err.to_string())),
    }
}

fn parse_codex_update_patch(
    workspace_root: &Path,
    patch_text: &str,
) -> Result<Vec<FilePatch>, String> {
    let lines: Vec<&str> = patch_text.lines().collect();
    let mut file_patches = Vec::new();
    let mut idx = 0;

    while idx < lines.len() {
        let line = lines[idx];
        if line.starts_with("*** Add File:")
            || line.starts_with("*** Delete File:")
            || line.starts_with("*** Move to:")
        {
            return Err("Use unified diff for add/delete/move patches.".to_string());
        }

        let Some(path) = line.strip_prefix("*** Update File:") else {
            idx += 1;
            continue;
        };
        let path = path.trim();
        if path.is_empty() {
            return Err("Missing path after *** Update File:.".to_string());
        }

        idx += 1;
        let mut hunks = Vec::new();
        while idx < lines.len() {
            let line = lines[idx];
            if line == "*** End Patch" || line.starts_with("*** Update File:") {
                break;
            }
            if line.starts_with("*** Add File:")
                || line.starts_with("*** Delete File:")
                || line.starts_with("*** Move to:")
            {
                return Err("Use unified diff for add/delete/move patches.".to_string());
            }
            if line.starts_with("@@") {
                idx += 1;
                let mut patch_lines = Vec::new();
                while idx < lines.len() {
                    let line = lines[idx];
                    if line.starts_with("@@") || line.starts_with("*** ") {
                        break;
                    }
                    if let Some(content) = line.strip_prefix(' ') {
                        patch_lines.push(PatchLine::Context(content.to_string()));
                    } else if let Some(content) = line.strip_prefix('-') {
                        patch_lines.push(PatchLine::Remove(content.to_string()));
                    } else if let Some(content) = line.strip_prefix('+') {
                        patch_lines.push(PatchLine::Add(content.to_string()));
                    } else if line.starts_with("\\ ") {
                    } else {
                        return Err(format!("Unsupported Codex patch line: {line}"));
                    }
                    idx += 1;
                }
                hunks.push(build_codex_hunk(workspace_root, path, patch_lines)?);
                continue;
            }
            idx += 1;
        }

        if hunks.is_empty() {
            return Err(format!("No hunks found for {path}."));
        }
        file_patches.push(FilePatch {
            old_path: PathBuf::from(path),
            new_path: PathBuf::from(path),
            hunks,
            is_new_file: false,
            is_delete: false,
        });
    }

    if file_patches.is_empty() {
        return Err("No *** Update File sections found.".to_string());
    }
    Ok(file_patches)
}

fn build_codex_hunk(
    workspace_root: &Path,
    path: &str,
    patch_lines: Vec<PatchLine>,
) -> Result<Hunk, String> {
    let consumed = patch_lines
        .iter()
        .filter(|line| matches!(line, PatchLine::Context(_) | PatchLine::Remove(_)))
        .count();
    if consumed == 0 {
        return Err("Codex update hunks need at least one context or removed line.".to_string());
    }

    let resolved =
        path::resolve_workspace_path(workspace_root, path).map_err(|err| err.to_string())?;
    let content = std::fs::read_to_string(&resolved).map_err(|err| err.to_string())?;
    let lines: Vec<String> = content.lines().map(str::to_string).collect();
    let old_start = find_codex_hunk_start(&lines, &patch_lines)
        .map(|offset| offset + 1)
        .ok_or_else(|| format!("Could not locate Codex apply_patch context in {path}."))?;
    let new_count = patch_lines
        .iter()
        .filter(|line| matches!(line, PatchLine::Context(_) | PatchLine::Add(_)))
        .count();

    Ok(Hunk {
        old_start,
        old_count: consumed,
        new_start: old_start,
        new_count,
        lines: patch_lines,
    })
}

fn find_codex_hunk_start(lines: &[String], patch_lines: &[PatchLine]) -> Option<usize> {
    let consumed = patch_lines
        .iter()
        .filter(|line| matches!(line, PatchLine::Context(_) | PatchLine::Remove(_)))
        .count();
    let max_start = lines.len().checked_sub(consumed)?;
    let mut match_offset = None;

    for offset in 0..=max_start {
        let mut file_idx = offset;
        let matched = patch_lines.iter().all(|patch_line| match patch_line {
            PatchLine::Context(expected) | PatchLine::Remove(expected) => {
                let matches = lines.get(file_idx) == Some(expected);
                file_idx += 1;
                matches
            }
            PatchLine::Add(_) => true,
        });

        if matched {
            if match_offset.is_some() {
                return None;
            }
            match_offset = Some(offset);
        }
    }

    match_offset
}

#[cfg(test)]
mod tests;

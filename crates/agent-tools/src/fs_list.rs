use crate::fs_helpers::resolve_workspace_read_path;
use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsListEntry {
    pub name: String,
    pub entry_type: String,
    pub size_bytes: u64,
    pub modified: String,
}

#[derive(Debug, Clone)]
pub struct FsListTool {
    workspace_root: PathBuf,
}

impl FsListTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl Tool for FsListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "fs.list".into(),
            description: "List directory contents within the workspace".into(),
            required_capability: "filesystem.read".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the directory within the workspace (default: .)"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Whether to list recursively (default: false)"
                    }
                }
            }),
        }
    }

    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk {
        let _ = invocation;
        ToolRisk::read("fs.list")
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let relative_path = invocation
            .arguments
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        let recursive = invocation
            .arguments
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let path = resolve_workspace_read_path(&self.workspace_root, relative_path)?;
        let canonical_root = self.workspace_root.canonicalize()?;

        if !path.is_dir() {
            return Err(crate::ToolError::ExecutionFailed(format!(
                "{} is not a directory",
                relative_path
            )));
        }

        let entries = if recursive {
            walk_dir(&path, &canonical_root)?
        } else {
            list_dir(&path, &canonical_root)?
        };

        let text = serde_json::to_string_pretty(&entries)
            .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolOutput {
            text,
            truncated: false,
            images: vec![],
        })
    }
}

/// List immediate children of a directory.
fn list_dir(dir: &Path, canonical_root: &Path) -> crate::Result<Vec<FsListEntry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        entries.push(make_list_entry(&entry, canonical_root)?);
    }
    sort_entries(&mut entries);
    Ok(entries)
}

/// Recursively walk a directory tree.
fn walk_dir(dir: &Path, canonical_root: &Path) -> crate::Result<Vec<FsListEntry>> {
    let mut entries = Vec::new();
    walk_dir_inner(dir, canonical_root, &mut entries)?;
    sort_entries(&mut entries);
    Ok(entries)
}

fn walk_dir_inner(
    dir: &Path,
    canonical_root: &Path,
    entries: &mut Vec<FsListEntry>,
) -> crate::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let is_dir = entry.file_type()?.is_dir();
        entries.push(make_list_entry(&entry, canonical_root)?);
        if is_dir {
            walk_dir_inner(&entry.path(), canonical_root, entries)?;
        }
    }
    Ok(())
}

fn make_list_entry(entry: &std::fs::DirEntry, canonical_root: &Path) -> crate::Result<FsListEntry> {
    let metadata = entry.metadata()?;
    let file_type = entry.file_type()?;
    let entry_type = if file_type.is_dir() {
        "dir"
    } else if file_type.is_symlink() {
        "symlink"
    } else {
        "file"
    }
    .to_string();

    let entry_path = entry.path();
    let name = entry_path
        .strip_prefix(canonical_root)
        .unwrap_or(&entry_path)
        .to_string_lossy()
        .to_string();

    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();

    Ok(FsListEntry {
        name,
        entry_type,
        size_bytes: metadata.len(),
        modified,
    })
}

/// Sort: directories first, then alphabetical by name.
fn sort_entries(entries: &mut [FsListEntry]) {
    entries.sort_by(|a, b| {
        let a_is_dir = a.entry_type == "dir";
        let b_is_dir = b.entry_type == "dir";
        b_is_dir.cmp(&a_is_dir).then_with(|| a.name.cmp(&b.name))
    });
}

#[cfg(test)]
#[path = "fs_list_tests.rs"]
mod tests;

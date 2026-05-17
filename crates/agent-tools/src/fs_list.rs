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
}

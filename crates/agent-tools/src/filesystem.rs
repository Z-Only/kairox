use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Module-level path validation
// ---------------------------------------------------------------------------

/// Validate a read path: the path must already exist and resolve inside the workspace.
pub fn resolve_workspace_read_path(
    workspace_root: &Path,
    relative_path: &str,
) -> crate::Result<PathBuf> {
    let candidate = workspace_root.join(relative_path);
    let root = workspace_root.canonicalize()?;
    let path = candidate.canonicalize()?;
    if path.starts_with(&root) {
        Ok(path)
    } else {
        Err(crate::ToolError::WorkspaceEscape(relative_path.into()))
    }
}

/// Validate a write path: the file may not exist yet, but the resolved path must
/// stay inside the workspace. Rejects paths containing `..`. If the file exists,
/// canonicalize and check. Otherwise, validate via the nearest existing parent.
pub fn resolve_workspace_write_path(
    workspace_root: &Path,
    relative_path: &str,
) -> crate::Result<PathBuf> {
    // Reject any relative path with ".." components to prevent traversal
    if relative_path.split(['/', '\\']).any(|c| c == "..") {
        return Err(crate::ToolError::WorkspaceEscape(relative_path.into()));
    }

    let root = workspace_root.canonicalize()?;
    let candidate = root.join(relative_path);

    if candidate.exists() {
        // File/dir exists — canonicalize and check containment
        let resolved = candidate.canonicalize()?;
        if resolved.starts_with(&root) {
            Ok(resolved)
        } else {
            Err(crate::ToolError::WorkspaceEscape(relative_path.into()))
        }
    } else {
        // File doesn't exist yet — validate via nearest existing parent
        let mut parent = candidate.parent();
        while let Some(p) = parent {
            if p.exists() {
                let resolved = p.canonicalize()?;
                if resolved.starts_with(&root) {
                    return Ok(candidate);
                } else {
                    return Err(crate::ToolError::WorkspaceEscape(relative_path.into()));
                }
            }
            parent = p.parent();
        }
        // No existing parent found within workspace — that's OK, create_dir_all
        // will be called later. The path is safe because we already rejected "..".
        Ok(candidate)
    }
}

// ---------------------------------------------------------------------------
// FsReadTool
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// FsWriteTool
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// FsListTool
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Tool, ToolInvocation};
    use std::io::Write as IoWrite;

    fn temp_workspace() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    /// Helper: canonicalize the workspace root to avoid macOS symlink issues
    /// (/var/folders → /private/var/folders).
    fn canon_root(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().canonicalize().unwrap()
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

    // -----------------------------------------------------------------------
    // resolve_workspace_read_path
    // -----------------------------------------------------------------------

    #[test]
    fn read_path_resolves_existing_file() {
        let dir = temp_workspace();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hi").unwrap();
        let resolved = resolve_workspace_read_path(&canon_root(&dir), "test.txt").unwrap();
        assert!(resolved.starts_with(canon_root(&dir)));
    }

    #[test]
    fn read_path_rejects_escape() {
        let dir = temp_workspace();
        let outside = dir.path().join("outside.txt");
        std::fs::write(&outside, "secret").unwrap();
        let workspace = dir.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let workspace = workspace.canonicalize().unwrap();
        let result = resolve_workspace_read_path(&workspace, "../outside.txt");
        assert!(result.is_err());
    }

    #[test]
    fn read_path_rejects_nonexistent() {
        let dir = temp_workspace();
        let result = resolve_workspace_read_path(&canon_root(&dir), "nope.txt");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // resolve_workspace_write_path
    // -----------------------------------------------------------------------

    #[test]
    fn write_path_allows_new_file_in_subdir() {
        let dir = temp_workspace();
        let root = canon_root(&dir);
        std::fs::create_dir(root.join("sub")).unwrap();
        let resolved = resolve_workspace_write_path(&root, "sub/new.txt").unwrap();
        assert_eq!(resolved, root.join("sub/new.txt"));
    }

    #[test]
    fn write_path_allows_new_file_in_root() {
        let dir = temp_workspace();
        let root = canon_root(&dir);
        let resolved = resolve_workspace_write_path(&root, "new.txt").unwrap();
        assert_eq!(resolved, root.join("new.txt"));
    }

    #[test]
    fn write_path_allows_overwrite_existing() {
        let dir = temp_workspace();
        let root = canon_root(&dir);
        std::fs::write(root.join("existing.txt"), "old").unwrap();
        let resolved = resolve_workspace_write_path(&root, "existing.txt").unwrap();
        assert!(resolved.starts_with(&root));
    }

    #[test]
    fn write_path_rejects_dot_dot() {
        let dir = temp_workspace();
        let root = canon_root(&dir);
        let result = resolve_workspace_write_path(&root, "../escape.txt");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("escape") || msg.contains("WorkspaceEscape"),
            "Expected escape error, got: {msg}"
        );
    }

    #[test]
    fn write_path_rejects_embedded_dot_dot() {
        let dir = temp_workspace();
        let root = canon_root(&dir);
        let result = resolve_workspace_write_path(&root, "sub/../../escape.txt");
        assert!(result.is_err());
    }

    #[test]
    fn write_path_allows_deep_new_file() {
        let dir = temp_workspace();
        let root = canon_root(&dir);
        // Neither sub/nor/deep exist yet; this should be OK since we rejected ".."
        let resolved = resolve_workspace_write_path(&root, "sub/nor/deep/file.txt").unwrap();
        assert_eq!(resolved, root.join("sub/nor/deep/file.txt"));
    }

    // -----------------------------------------------------------------------
    // FsReadTool tests (existing, adapted)
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // FsWriteTool tests
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // FsListTool tests
    // -----------------------------------------------------------------------

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

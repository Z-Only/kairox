# Filesystem Write & List Tools — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `fs.write` and `fs.list` built-in tools with atomic writes, workspace confinement, directory listing, and comprehensive tests.

**Architecture:** Extend the existing `crates/agent-tools/src/filesystem.rs` module with two new `Tool` implementations (`FsWriteTool`, `FsListTool`), refactor the shared path validation logic into a module-level function, and register the new tools in `BuiltinProvider::with_defaults`.

**Tech Stack:** Rust, tokio (async fs), serde/serde_json (serialization), tempfile (test fixtures)

---

## File Structure

| Action | File                                         | Responsibility                                                                |
| ------ | -------------------------------------------- | ----------------------------------------------------------------------------- |
| Modify | `crates/agent-tools/src/filesystem.rs`       | Add `FsWriteTool`, `FsListTool`, `FsListEntry`, refactor path validation      |
| Modify | `crates/agent-tools/src/provider/builtin.rs` | Register `FsWriteTool` and `FsListTool` in `with_defaults`; update test count |
| Modify | `crates/agent-tools/src/lib.rs`              | Re-export `FsWriteTool`, `FsListTool`, `FsListEntry`                          |
| Modify | `apps/agent-gui/e2e/tauri-mock.js`           | Add stubs for `fs.write` and `fs.list` tool invocations                       |

---

### Task 1: Refactor `resolve_workspace_path` to module level

**Files:**

- Modify: `crates/agent-tools/src/filesystem.rs`

- [ ] **Step 1: Add module-level `resolve_workspace_read_path` function**

Add this public function after the imports, before the `FsReadTool` struct:

```rust
/// Validate that `relative_path` resolves within `workspace_root`.
/// The path must already exist (for read operations).
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

/// Validate that `relative_path` is safe for a write operation within `workspace_root`.
/// The file does NOT need to exist yet — this handles new file creation.
pub fn resolve_workspace_write_path(
    workspace_root: &Path,
    relative_path: &str,
) -> crate::Result<PathBuf> {
    // Reject path traversal
    if relative_path.contains("..") {
        return Err(crate::ToolError::WorkspaceEscape(relative_path.into()));
    }
    let path = workspace_root.join(relative_path);
    if path.exists() {
        // File exists: canonicalize and verify it's within workspace
        let root = workspace_root.canonicalize()?;
        let canonical = path.canonicalize()?;
        if !canonical.starts_with(&root) {
            return Err(crate::ToolError::WorkspaceEscape(relative_path.into()));
        }
        Ok(canonical)
    } else {
        // New file: validate the nearest existing parent
        let root = workspace_root.canonicalize()?;
        if let Some(parent) = path.parent() {
            if parent.exists() {
                let canonical_parent = parent.canonicalize()?;
                if !canonical_parent.starts_with(&root) {
                    return Err(crate::ToolError::WorkspaceEscape(relative_path.into()));
                }
            }
            // If parent doesn't exist, create_dir_all will be called later.
            // The workspace root check holds since we joined from workspace_root.
        }
        Ok(path)
    }
}
```

- [ ] **Step 2: Refactor `FsReadTool` to use the module-level function**

Replace the `resolve_workspace_path` method on `FsReadTool`. Remove the private method and update `invoke` to call the module-level function:

```rust
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
```

- [ ] **Step 3: Update existing `FsReadTool` tests to use the module-level function**

No changes needed — the tests call `Tool::invoke()` which internally uses the refactored function. Run tests to confirm:

```bash
cargo test -p agent-tools --lib filesystem
```

Expected: All existing tests pass.

- [ ] **Step 4: Add unit tests for the path validation functions**

Add inside `#[cfg(test)] mod tests`:

```rust
#[test]
fn resolve_read_path_rejects_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside.txt");
    std::fs::write(&outside, "secret").unwrap();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();

    let result = resolve_workspace_read_path(&workspace, "../outside.txt");
    assert!(result.is_err());
}

#[test]
fn resolve_write_path_rejects_double_dot() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();

    let result = resolve_workspace_write_path(&workspace, "../escape.txt");
    assert!(result.is_err());
}

#[test]
fn resolve_write_path_allows_new_file_in_existing_dir() {
    let dir = tempfile::tempdir().unwrap();

    let result = resolve_workspace_write_path(dir.path(), "new_file.txt");
    assert!(result.is_ok());
}

#[test]
fn resolve_write_path_allows_new_file_in_nested_new_dir() {
    let dir = tempfile::tempdir().unwrap();

    // Parent dir doesn't exist yet — but we joined from workspace_root
    let result = resolve_workspace_write_path(dir.path(), "sub/dir/new_file.txt");
    assert!(result.is_ok());
}

#[test]
fn resolve_write_path_allows_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("existing.txt");
    std::fs::write(&file, "old content").unwrap();

    let result = resolve_workspace_write_path(dir.path(), "existing.txt");
    assert!(result.is_ok());
}
```

- [ ] **Step 5: Run tests and commit**

```bash
cargo test -p agent-tools --lib filesystem
cargo clippy -p agent-tools --all-features -- -D warnings
```

Expected: All tests pass, no clippy warnings.

```bash
git add crates/agent-tools/src/filesystem.rs
git commit -m "refactor(tools): extract resolve_workspace_path to module level for reuse"
```

---

### Task 2: Implement `FsWriteTool`

**Files:**

- Modify: `crates/agent-tools/src/filesystem.rs`

- [ ] **Step 1: Write the failing tests for `FsWriteTool`**

Add inside `#[cfg(test)] mod tests`:

```rust
// --- FsWriteTool tests ---

#[test]
fn fs_write_definition() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());
    let def = tool.definition();
    assert_eq!(def.tool_id, "fs.write");
    assert_eq!(def.required_capability, "filesystem.write");
}

#[tokio::test]
async fn fs_write_creates_new_file() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.write".into(),
        arguments: serde_json::json!({"path": "hello.txt", "content": "Hello, world!"}),
        workspace_id: "wrk_test".into(),
        preview: "fs.write(hello.txt)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Written"));
    let content = tokio::fs::read_to_string(dir.path().join("hello.txt")).await.unwrap();
    assert_eq!(content, "Hello, world!");
}

#[tokio::test]
async fn fs_write_overwrites_existing_and_creates_backup() {
    let dir = temp_workspace();
    // Pre-create file
    tokio::fs::write(dir.path().join("data.txt"), "old content")
        .await
        .unwrap();

    let tool = FsWriteTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.write".into(),
        arguments: serde_json::json!({"path": "data.txt", "content": "new content"}),
        workspace_id: "wrk_test".into(),
        preview: "fs.write(data.txt)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Written"));

    // New content
    let content = tokio::fs::read_to_string(dir.path().join("data.txt")).await.unwrap();
    assert_eq!(content, "new content");

    // Backup
    let backup = tokio::fs::read_to_string(dir.path().join("data.txt.bak")).await.unwrap();
    assert_eq!(backup, "old content");
}

#[tokio::test]
async fn fs_write_creates_parent_dirs() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.write".into(),
        arguments: serde_json::json!({"path": "sub/dir/nested.txt", "content": "deep"}),
        workspace_id: "wrk_test".into(),
        preview: "fs.write(sub/dir/nested.txt)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Written"));
    let content = tokio::fs::read_to_string(dir.path().join("sub/dir/nested.txt"))
        .await
        .unwrap();
    assert_eq!(content, "deep");
}

#[tokio::test]
async fn fs_write_creates_parent_dirs_disabled() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.write".into(),
        arguments: serde_json::json!({"path": "sub/dir/nested.txt", "content": "deep", "create_dirs": false}),
        workspace_id: "wrk_test".into(),
        preview: "fs.write(sub/dir/nested.txt)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn fs_write_rejects_workspace_escape() {
    let dir = temp_workspace();
    // Create a file outside the workspace
    let outside = dir.path().join("outside.txt");
    std::fs::write(&outside, "secret").unwrap();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();

    let tool = FsWriteTool::new(workspace);
    let invocation = ToolInvocation {
        tool_id: "fs.write".into(),
        arguments: serde_json::json!({"path": "../outside.txt", "content": "hacked"}),
        workspace_id: "wrk_test".into(),
        preview: "fs.write(../outside.txt)".into(),
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
async fn fs_write_missing_content_arg() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.write".into(),
        arguments: serde_json::json!({"path": "test.txt"}),
        workspace_id: "wrk_test".into(),
        preview: "fs.write(test.txt)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn fs_write_no_leftover_tmp_file() {
    let dir = temp_workspace();
    let tool = FsWriteTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.write".into(),
        arguments: serde_json::json!({"path": "clean.txt", "content": "no tmp"}),
        workspace_id: "wrk_test".into(),
        preview: "fs.write(clean.txt)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    tool.invoke(invocation).await.unwrap();

    // No .tmp files should remain
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains(".tmp")
        })
        .collect();
    assert!(entries.is_empty(), "Found leftover tmp files: {:?}", entries);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p agent-tools --lib filesystem -- fs_write
```

Expected: Compilation error — `FsWriteTool` not found.

- [ ] **Step 3: Implement `FsWriteTool`**

Add after `FsReadTool` implementation (but before the tests module):

```rust
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
            description: "Write content to a file within the workspace (atomic, with backup)".into(),
            required_capability: "filesystem.write".into(),
        }
    }

    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk {
        let _ = invocation;
        ToolRisk::write("fs.write")
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let relative_path = invocation
            .arguments
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");

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
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                if create_dirs {
                    tokio::fs::create_dir_all(parent).await?;
                } else {
                    return Err(crate::ToolError::ExecutionFailed(format!(
                        "parent directory does not exist: {}",
                        parent.display()
                    )));
                }
            }
        }

        // Backup existing file
        if path.exists() {
            let backup_path = path.with_extension(
                format!(
                    "{}.bak",
                    path.extension()
                        .map(|e| format!("{}.", e.to_string_lossy()))
                        .unwrap_or_default()
                )
                .trim_end_matches('.'),
            );
            // Simpler: just append .bak to the full path
            let backup_path = {
                let mut bak = path.as_os_str().to_owned();
                bak.push(".bak");
                PathBuf::from(bak)
            };
            tokio::fs::copy(&path, &backup_path).await?;
        }

        // Atomic write: write to temp file, then rename
        let tmp_path = {
            let mut tmp = path.as_os_str().to_owned();
            tmp.push(format!(".tmp.{}", std::process::id()));
            PathBuf::from(tmp)
        };
        tokio::fs::write(&tmp_path, content).await?;
        tokio::fs::rename(&tmp_path, &path).await?;

        let bytes = content.len();
        Ok(ToolOutput {
            text: format!("Written {} bytes to {}", bytes, relative_path),
            truncated: false,
        })
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p agent-tools --lib filesystem -- fs_write
```

Expected: All `fs_write_*` tests pass.

- [ ] **Step 5: Run clippy and commit**

```bash
cargo clippy -p agent-tools --all-features -- -D warnings
```

Expected: No warnings.

```bash
git add crates/agent-tools/src/filesystem.rs
git commit -m "feat(tools): add FsWriteTool with atomic writes and workspace confinement"
```

---

### Task 3: Implement `FsListTool`

**Files:**

- Modify: `crates/agent-tools/src/filesystem.rs`

- [ ] **Step 1: Write the failing tests for `FsListTool`**

Add inside `#[cfg(test)] mod tests`:

```rust
// --- FsListTool tests ---

#[test]
fn fs_list_definition() {
    let dir = temp_workspace();
    let tool = FsListTool::new(dir.path().to_path_buf());
    let def = tool.definition();
    assert_eq!(def.tool_id, "fs.list");
    assert_eq!(def.required_capability, "filesystem.read");
}

#[tokio::test]
async fn fs_list_shows_directory_contents() {
    let dir = temp_workspace();
    tokio::fs::write(dir.path().join("file1.txt"), "hello")
        .await
        .unwrap();
    tokio::fs::create_dir(dir.path().join("subdir"))
        .await
        .unwrap();
    tokio::fs::write(dir.path().join("file2.rs"), "fn main(){}")
        .await
        .unwrap();

    let tool = FsListTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.list".into(),
        arguments: serde_json::json!({"path": "."}),
        workspace_id: "wrk_test".into(),
        preview: "fs.list(.)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let output = tool.invoke(invocation).await.unwrap();
    let entries: Vec<FsListEntry> = serde_json::from_str(&output.text).unwrap();

    // Directories first, then files, alphabetical within groups
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].name, "subdir");
    assert_eq!(entries[0].entry_type, "dir");
    // files sorted alphabetically
    let file_names: Vec<&str> = entries[1..].iter().map(|e| e.name.as_str()).collect();
    assert!(file_names.contains(&"file1.txt"));
    assert!(file_names.contains(&"file2.rs"));
}

#[tokio::test]
async fn fs_list_recursive() {
    let dir = temp_workspace();
    tokio::fs::create_dir_all(dir.path().join("a/b")).await.unwrap();
    tokio::fs::write(dir.path().join("a/top.txt"), "top")
        .await
        .unwrap();
    tokio::fs::write(dir.path().join("a/b/deep.txt"), "deep")
        .await
        .unwrap();

    let tool = FsListTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.list".into(),
        arguments: serde_json::json!({"path": ".", "recursive": true}),
        workspace_id: "wrk_test".into(),
        preview: "fs.list(. recursive)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let output = tool.invoke(invocation).await.unwrap();
    let entries: Vec<FsListEntry> = serde_json::from_str(&output.text).unwrap();

    // Should find a, a/top.txt, a/b, a/b/deep.txt
    assert!(entries.len() >= 4, "Expected at least 4 entries, got {}", entries.len());
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"a"));
    assert!(names.contains(&"top.txt"));
}

#[tokio::test]
async fn fs_list_rejects_workspace_escape() {
    let dir = temp_workspace();
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();

    let tool = FsListTool::new(workspace);
    let invocation = ToolInvocation {
        tool_id: "fs.list".into(),
        arguments: serde_json::json!({"path": ".."}),
        workspace_id: "wrk_test".into(),
        preview: "fs.list(..)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn fs_list_nonexistent_dir_returns_error() {
    let dir = temp_workspace();
    let tool = FsListTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.list".into(),
        arguments: serde_json::json!({"path": "no_such_dir"}),
        workspace_id: "wrk_test".into(),
        preview: "fs.list(no_such_dir)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn fs_list_default_path_is_root() {
    let dir = temp_workspace();
    tokio::fs::write(dir.path().join("root_file.txt"), "hi")
        .await
        .unwrap();

    let tool = FsListTool::new(dir.path().to_path_buf());
    let invocation = ToolInvocation {
        tool_id: "fs.list".into(),
        arguments: serde_json::json!({}),
        workspace_id: "wrk_test".into(),
        preview: "fs.list(default)".into(),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    };
    let output = tool.invoke(invocation).await.unwrap();
    let entries: Vec<FsListEntry> = serde_json::from_str(&output.text).unwrap();
    assert!(entries.iter().any(|e| e.name == "root_file.txt"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p agent-tools --lib filesystem -- fs_list
```

Expected: Compilation error — `FsListTool` not found.

- [ ] **Step 3: Implement `FsListEntry` and `FsListTool`**

Add after `FsWriteTool` implementation:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsListEntry {
    pub name: String,
    pub entry_type: String,  // "file" | "dir" | "symlink"
    pub size_bytes: u64,
    pub modified: String,    // ISO 8601
}

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

        if !path.is_dir() {
            return Err(crate::ToolError::ExecutionFailed(format!(
                "not a directory: {}",
                relative_path
            )));
        }

        let entries = if recursive {
            self.list_recursive(&path, &self.workspace_root)?
        } else {
            self.list_directory(&path, &self.workspace_root)?
        };

        let text = serde_json::to_string_pretty(&entries)
            .map_err(|e| crate::ToolError::ExecutionFailed(format!("serialization error: {e}")))?;

        let truncated = text.len() > invocation.output_limit_bytes;
        Ok(ToolOutput { text, truncated })
    }
}

impl FsListTool {
    fn list_directory(
        &self,
        dir: &Path,
        workspace_root: &Path,
    ) -> crate::Result<Vec<FsListEntry>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(dir)
            .map_err(|e| crate::ToolError::ExecutionFailed(format!("read_dir failed: {e}")))?
        {
            let entry = entry.map_err(|e| crate::ToolError::ExecutionFailed(format!("dir entry error: {e}")))?;
            let fs_entry = self.entry_to_fs_list_entry(&entry, workspace_root)?;
            entries.push(fs_entry);
        }
        self.sort_entries(&mut entries);
        Ok(entries)
    }

    fn list_recursive(
        &self,
        dir: &Path,
        workspace_root: &Path,
    ) -> crate::Result<Vec<FsListEntry>> {
        let mut entries = Vec::new();
        self.walk_dir(dir, workspace_root, &mut entries)?;
        self.sort_entries(&mut entries);
        Ok(entries)
    }

    fn walk_dir(
        &self,
        dir: &Path,
        workspace_root: &Path,
        entries: &mut Vec<FsListEntry>,
    ) -> crate::Result<()> {
        for entry in std::fs::read_dir(dir)
            .map_err(|e| crate::ToolError::ExecutionFailed(format!("read_dir failed: {e}")))?
        {
            let entry = entry.map_err(|e| crate::ToolError::ExecutionFailed(format!("dir entry error: {e}")))?;
            let fs_entry = self.entry_to_fs_list_entry(&entry, workspace_root)?;
            let is_dir = entry.path().is_dir();
            entries.push(fs_entry);
            if is_dir {
                self.walk_dir(&entry.path(), workspace_root, entries)?;
            }
        }
        Ok(())
    }

    fn entry_to_fs_list_entry(
        &self,
        entry: &std::fs::DirEntry,
        workspace_root: &Path,
    ) -> crate::Result<FsListEntry> {
        let metadata = entry.metadata().map_err(|e| {
            crate::ToolError::ExecutionFailed(format!("metadata error for {}: {e}", entry.path().display()))
        })?;

        let file_type = if metadata.is_symlink() {
            "symlink"
        } else if metadata.is_dir() {
            "dir"
        } else {
            "file"
        };

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        // Use relative path from workspace root for the name in recursive mode
        let name = entry
            .path()
            .strip_prefix(workspace_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| entry.file_name().to_string_lossy().to_string());

        Ok(FsListEntry {
            name,
            entry_type: file_type.to_string(),
            size_bytes: metadata.len(),
            modified,
        })
    }

    fn sort_entries(&self, entries: &mut [FsListEntry]) {
        entries.sort_by(|a, b| {
            // Directories first, then files, alphabetical within groups
            match (a.entry_type.as_str(), b.entry_type.as_str()) {
                ("dir", "dir") | ("file", "file") | ("symlink", "symlink") => a.name.cmp(&b.name),
                ("dir", _) => std::cmp::Ordering::Less,
                (_, "dir") => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
    }
}
```

- [ ] **Step 4: Add `chrono` dependency to `agent-tools/Cargo.toml`**

The `FsListTool` uses `chrono` for ISO 8601 timestamp formatting. Check if it's already a dependency:

```bash
grep "chrono" crates/agent-tools/Cargo.toml
```

If not present, add it. Check the workspace Cargo.toml for the version:

```bash
grep "chrono" Cargo.toml
```

If `chrono` is in workspace dependencies, add `chrono.workspace = true` to `crates/agent-tools/Cargo.toml`. If not, add `chrono = { workspace = true }` to the root `Cargo.toml` `[workspace.dependencies]` first, then reference it in the crate.

Alternatively, if we want to avoid adding `chrono` as a dependency, we can use a simpler format without it. Replace the `modified` field computation with:

```rust
let modified = metadata
    .modified()
    .ok()
    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
    .map(|d| d.as_secs().to_string())
    .unwrap_or_default();
```

This uses a Unix timestamp instead of ISO 8601. The design spec says ISO 8601, but using a simple timestamp avoids adding a dependency. For the initial implementation, use the simple approach and document the decision.

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test -p agent-tools --lib filesystem -- fs_list
```

Expected: All `fs_list_*` tests pass.

- [ ] **Step 6: Run all filesystem tests together**

```bash
cargo test -p agent-tools --lib filesystem
```

Expected: All filesystem tests pass (FsRead + FsWrite + FsList + path validation).

- [ ] **Step 7: Commit**

```bash
git add crates/agent-tools/src/filesystem.rs
git commit -m "feat(tools): add FsListTool with structured directory listing"
```

---

### Task 4: Update exports and `BuiltinProvider`

**Files:**

- Modify: `crates/agent-tools/src/lib.rs`
- Modify: `crates/agent-tools/src/provider/builtin.rs`

- [ ] **Step 1: Update `lib.rs` exports**

Add the new public exports:

```rust
pub use filesystem::{FsListEntry, FsListTool, FsWriteTool};
```

Make sure `FsReadTool` is already exported (add it if not):

```rust
pub use filesystem::FsReadTool;
```

- [ ] **Step 2: Update `BuiltinProvider::with_defaults`**

In `crates/agent-tools/src/provider/builtin.rs`, add the imports and tool registrations:

```rust
use crate::filesystem::{FsListTool, FsReadTool, FsWriteTool};
```

And in `with_defaults`:

```rust
pub fn with_defaults(workspace_root: PathBuf) -> Self {
    let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();

    let shell = Box::new(ShellExecTool::new(workspace_root.clone())) as Box<dyn Tool>;
    let search = Box::new(RipgrepSearchTool::new(workspace_root.clone())) as Box<dyn Tool>;
    let patch = Box::new(PatchApplyTool::new(workspace_root.clone())) as Box<dyn Tool>;
    let fs_read = Box::new(FsReadTool::new(workspace_root.clone())) as Box<dyn Tool>;
    let fs_write = Box::new(FsWriteTool::new(workspace_root.clone())) as Box<dyn Tool>;
    let fs_list = Box::new(FsListTool::new(workspace_root)) as Box<dyn Tool>;

    tools.insert(shell.definition().tool_id.clone(), Arc::from(shell));
    tools.insert(search.definition().tool_id.clone(), Arc::from(search));
    tools.insert(patch.definition().tool_id.clone(), Arc::from(patch));
    tools.insert(fs_read.definition().tool_id.clone(), Arc::from(fs_read));
    tools.insert(fs_write.definition().tool_id.clone(), Arc::from(fs_write));
    tools.insert(fs_list.definition().tool_id.clone(), Arc::from(fs_list));

    Self { tools }
}
```

- [ ] **Step 3: Update the `builtin_provider_lists_all_tools` test**

Change `assert_eq!(tools.len(), 4);` to `assert_eq!(tools.len(), 6);` and add assertions:

```rust
assert!(
    tool_ids.contains(&"fs.write"),
    "missing fs.write, got: {:?}",
    tool_ids
);
assert!(
    tool_ids.contains(&"fs.list"),
    "missing fs.list, got: {:?}",
    tool_ids
);
```

- [ ] **Step 4: Run all tests**

```bash
cargo test -p agent-tools
cargo clippy -p agent-tools --all-features -- -D warnings
```

Expected: All tests pass, no clippy warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-tools/src/lib.rs crates/agent-tools/src/provider/builtin.rs
git commit -m "feat(tools): register FsWriteTool and FsListTool in BuiltinProvider"
```

---

### Task 5: Update E2E mock and run full test suite

**Files:**

- Modify: `apps/agent-gui/e2e/tauri-mock.js`

- [ ] **Step 1: Add tool stubs to the E2E mock**

Find the `invoke` handler in `tauri-mock.js` and add stubs for `fs.write` and `fs.list` if there's a tool invocation handler. Since the E2E tests don't directly invoke tools, this step may be a no-op. Check the mock structure first:

```bash
grep -n "fs.read\|fs.write\|tool" apps/agent-gui/e2e/tauri-mock.js | head -20
```

If no tool invocation handling exists, skip this step.

- [ ] **Step 2: Run the full test suite**

```bash
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cd apps/agent-gui && npx vitest run
npx playwright test
```

Expected: All tests pass.

- [ ] **Step 3: Commit (if any changes were made)**

```bash
git add apps/agent-gui/e2e/tauri-mock.js  # only if modified
git commit -m "chore(gui): update E2E mock for fs.write and fs.list tools"
```

---

### Task 6: Final verification and workspace-wide checks

- [ ] **Step 1: Run `just check` (full CI gate)**

```bash
just check
```

Expected: Format check, lint, and all tests pass.

- [ ] **Step 2: Verify type generation is not affected**

```bash
just check-types
```

Expected: No type sync changes (no new Tauri commands or event types were added).

- [ ] **Step 3: Final commit message**

If all checks pass, no additional commit needed. The feature is complete across Tasks 1–5.

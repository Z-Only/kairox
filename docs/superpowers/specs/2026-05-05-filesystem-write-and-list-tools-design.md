# Filesystem Write & List Tools — Design Spec

**Date:** 2026-05-05
**Status:** Approved
**Scope:** Add `fs.write` and `fs.list` built-in tools to `agent-tools`, wire into `BuiltinProvider` and runtime, update E2E mock, add integration tests.

---

## Problem

The Kairox agent currently has no dedicated file write capability. The Phase 1 workbench design spec lists "Filesystem read and write within workspace policy" as a tool family, but only `fs.read` was implemented. The consequences:

1. **Agents cannot reliably create or edit files** — they must rely on `shell.exec` (e.g., `echo "content" > file`) or `patch.apply`, both of which are lossy for direct writes.
2. **No atomic write guarantee** — `shell.exec` writes are non-atomic; a crash mid-write leaves a corrupted file.
3. **No workspace-constrained write validation** — `shell.exec` can write anywhere the process has permission; `fs.write` would enforce workspace boundary confinement, same as `fs.read`.
4. **Permission gating is incomplete** — `ToolRisk::write("fs.write")` is already tested in `permission.rs` but there's no backing tool to exercise it in production.
5. **No directory listing tool** — Agents use `shell.exec` with `ls` to explore directory structure, but the output is unstructured text. A `fs.list` tool returns structured JSON with file type, size, and modified time.

## Goal

Add two new built-in tools:

1. **`fs.write`** — Atomic, workspace-confined file write with parent directory creation and backup.
2. **`fs.list`** — Workspace-confined directory listing with structured output (name, type, size, modified time).

Both tools follow existing patterns (workspace path validation, `ToolRisk` classification, `ToolInvocation` / `ToolOutput` protocol) and register in `BuiltinProvider::with_defaults`.

## Design Decisions

| Decision                  | Choice                                                        | Rationale                                                                               |
| ------------------------- | ------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Write strategy            | Write to temp file, then `rename`                             | Atomic on POSIX; no partial writes on crash                                             |
| Backup on overwrite       | Create `.bak` before overwriting existing files               | Simple undo for single-file writes; doesn't require full VCS                            |
| Parent directory creation | `create_dir_all` if parent doesn't exist                      | Matches developer expectations; `patch.apply` already does this                         |
| Path validation           | Same `resolve_workspace_path` pattern as `fs.read`            | DRY; reuses proven sandbox logic                                                        |
| Line ending normalization | No normalization; write content as-is                         | Agent produces the content it wants; don't second-guess                                 |
| List output format        | JSON array of `FsEntry` objects                               | Structured for model consumption; not dependent on `ls` formatting                      |
| List recursion            | Flat (single directory) by default, optional `recursive` flag | Most agent queries need top-level structure; recursive available for deeper exploration |
| List symlinks             | Follow symlinks but report as type "symlink"                  | Transparent but not misleading                                                          |
| `fs.write` risk           | `ToolRisk::write("fs.write")`                                 | Already defined in permission engine                                                    |
| `fs.list` risk            | `ToolRisk::read("fs.list")`                                   | Read-only operation                                                                     |

## Detailed Design

### 1. `fs.write` Tool

**Location:** `crates/agent-tools/src/filesystem.rs` (extend existing module)

```rust
pub struct FsWriteTool {
    workspace_root: PathBuf,
}

impl FsWriteTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}
```

**Tool definition:**

- `tool_id`: `"fs.write"`
- `description`: `"Write content to a file within the workspace (atomic, with backup)"`
- `required_capability`: `"filesystem.write"`

**Arguments (JSON):**

```json
{
  "path": "relative/path/to/file.txt",
  "content": "file content as string",
  "create_dirs": true // optional, default true
}
```

**Risk:** `ToolRisk::write("fs.write")`

**Invoke logic:**

1. Extract `path` and `content` from `invocation.arguments`.
2. Validate `create_dirs` (default `true`).
3. Resolve workspace path via `resolve_workspace_path()` (reuse from `FsReadTool` — extract to module-level).
4. If file exists, create backup at `<path>.bak` (overwrite previous backup).
5. Create parent directories if `create_dirs` is true and they don't exist.
6. Write to `<path>.tmp.<pid>` (temp file in same directory).
7. `rename` temp file to target path (atomic on same filesystem).
8. Return `ToolOutput` with summary: `"Written <n> bytes to <path>"`.

**Error cases:**

- `WorkspaceEscape` — path escapes workspace root
- `Io` — permission denied, disk full, etc.
- `ExecutionFailed` — `content` argument missing or not a string

### 2. `FsListEntry` Struct and `fs.list` Tool

**Location:** `crates/agent-tools/src/filesystem.rs`

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
```

**Tool definition:**

- `tool_id`: `"fs.list"`
- `description`: `"List directory contents within the workspace"`
- `required_capability`: `"filesystem.read"`

**Arguments (JSON):**

```json
{
  "path": "relative/path/to/dir",
  "recursive": false // optional, default false
}
```

**Risk:** `ToolRisk::read("fs.list")`

**Invoke logic:**

1. Extract `path` from `invocation.arguments` (default `"."`).
2. Resolve workspace path.
3. If `recursive` is true, walk directory tree; otherwise read single directory.
4. For each entry, collect `FsListEntry` with name, type, size, modified time.
5. Sort entries: directories first, then files, alphabetical within groups.
6. Return JSON-serialized `Vec<FsListEntry>` as `ToolOutput.text`.

**Error cases:**

- `WorkspaceEscape` — path escapes workspace root
- `ExecutionFailed` — path is not a directory
- `Io` — permission denied

### 3. Refactor: Extract `resolve_workspace_path`

Both `FsReadTool` and `FsWriteTool` (and `FsListTool`) need the same workspace path validation. Extract it to a module-level function:

```rust
fn resolve_workspace_path(workspace_root: &Path, relative_path: &str) -> crate::Result<PathBuf> {
    let candidate = workspace_root.join(relative_path);
    let root = workspace_root.canonicalize()?;
    let path = candidate.canonicalize()?;
    if path.starts_with(&root) {
        Ok(path)
    } else {
        Err(crate::ToolError::WorkspaceEscape(relative_path.into()))
    }
}
```

Change `FsReadTool::resolve_workspace_path` to call this. Handle the case where the path doesn't exist yet (for `fs.write`, the file may not exist, so `canonicalize` would fail — use a different resolution strategy for new files).

**New file path resolution for writes:**

```rust
fn resolve_workspace_write_path(workspace_root: &Path, relative_path: &str) -> crate::Result<PathBuf> {
    // Reject obviously malicious paths
    if relative_path.contains("..") {
        return Err(crate::ToolError::WorkspaceEscape(relative_path.into()));
    }
    let path = workspace_root.join(relative_path);
    // If the file exists, canonicalize and validate
    if path.exists() {
        let root = workspace_root.canonicalize()?;
        let canonical = path.canonicalize()?;
        if !canonical.starts_with(&root) {
            return Err(crate::ToolError::WorkspaceEscape(relative_path.into()));
        }
        Ok(canonical)
    } else {
        // New file: validate parent directory
        if let Some(parent) = path.parent() {
            if parent.exists() {
                let root = workspace_root.canonicalize()?;
                let canonical_parent = parent.canonicalize()?;
                if !canonical_parent.starts_with(&root) {
                    return Err(crate::ToolError::WorkspaceEscape(relative_path.into()));
                }
            }
            // If parent doesn't exist either, create_dir_all will be called
            // The workspace root check still holds since we joined from workspace_root
        }
        Ok(path)
    }
}
```

### 4. Register in `BuiltinProvider`

Update `crates/agent-tools/src/provider/builtin.rs`:

```rust
impl BuiltinProvider {
    pub fn with_defaults(workspace_root: PathBuf) -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();

        let shell = Box::new(ShellExecTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let search = Box::new(RipgrepSearchTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let patch = Box::new(PatchApplyTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let fs_read = Box::new(FsReadTool::new(workspace_root.clone())) as Box<dyn Tool>;
        let fs_write = Box::new(FsWriteTool::new(workspace_root.clone())) as Box<dyn Tool>;  // NEW
        let fs_list = Box::new(FsListTool::new(workspace_root.clone())) as Box<dyn Tool>;    // NEW

        tools.insert(shell.definition().tool_id.clone(), Arc::from(shell));
        tools.insert(search.definition().tool_id.clone(), Arc::from(search));
        tools.insert(patch.definition().tool_id.clone(), Arc::from(patch));
        tools.insert(fs_read.definition().tool_id.clone(), Arc::from(fs_read));
        tools.insert(fs_write.definition().tool_id.clone(), Arc::from(fs_write));  // NEW
        tools.insert(fs_list.definition().tool_id.clone(), Arc::from(fs_list));    // NEW

        Self { tools }
    }
}
```

### 5. No changes to `agent-core`, `agent-runtime`, or GUI

The tools are registered automatically via `BuiltinProvider::with_defaults`, which is already wired in `facade_runtime.rs`. No changes needed to:

- `agent-core` — no new event types
- `agent-runtime` — tool registry is dynamic
- Tauri commands — no new IPC surface
- GUI components — trace and permission UI already handle generic tool invocations

### 6. Update E2E Mock

Update `apps/agent-gui/e2e/tauri-mock.js` to handle `fs.write` and `fs.list` tool invocations if they appear in test scenarios. Since current E2E tests don't exercise tool calls directly, this is a low-priority change — add stubs that return success.

### 7. Tests

**Unit tests in `filesystem.rs`:**

| Test                                    | Description                                              |
| --------------------------------------- | -------------------------------------------------------- |
| `fs_write_definition`                   | Verify tool_id, description, capability                  |
| `fs_write_creates_new_file`             | Write to new path, verify content                        |
| `fs_write_overwrites_existing`          | Write to existing path, verify .bak created, new content |
| `fs_write_creates_parent_dirs`          | Write to nested path with `create_dirs: true`            |
| `fs_write_rejects_workspace_escape`     | `../outside.txt` returns WorkspaceEscape error           |
| `fs_write_atomic_no_partial`            | Verify no .tmp file remains after success                |
| `fs_write_missing_content_arg`          | Returns error when `content` is missing                  |
| `fs_list_shows_directory_contents`      | List a directory with files and subdirs                  |
| `fs_list_sorts_dirs_first`              | Verify directories before files                          |
| `fs_list_recursive`                     | Recursive listing with nested dirs                       |
| `fs_list_rejects_workspace_escape`      | `../` path returns error                                 |
| `fs_list_nonexistent_dir`               | Returns error for missing directory                      |
| `resolve_workspace_path_existing`       | Existing file resolves within workspace                  |
| `resolve_workspace_write_path_new_file` | New file path resolves correctly                         |

**Integration test in `full_stack.rs`:**

Add a test `tool_fs_write_and_read_roundtrip` that:

1. Creates a `LocalRuntime` with `FsWriteTool` and `FsReadTool` registered.
2. Invokes `fs.write` with content.
3. Invokes `fs.read` to verify the content.
4. Invokes `fs.list` to verify the file appears.
5. Invokes `fs.write` again to overwrite.
6. Verifies `.bak` file exists with original content.

**Update existing `BuiltinProvider` test:**

The test `builtin_provider_lists_all_tools` currently asserts `tools.len() == 4`. Update to `== 6`.

## Crate Dependency Changes

None. `filesystem.rs` already uses `tokio::fs`, `serde`, `serde_json`, `tempfile` (dev-dep).

## Migration Notes

- No breaking changes. `fs.write` and `fs.list` are additive tools.
- `BuiltinProvider::with_defaults` test count changes from 4 to 6.
- Existing `patch.apply` still works for diff-based edits; `fs.write` is for full-file replacement/creation.

# Tool Provider & Builtin Tools Design

Date: 2026-04-30
Status: Approved
Scope: agent-tools crate, agent-runtime integration, agent-core event extensions

## Context

Kairox v0.2.0 has a working Agent Loop (model → tool call → permission → execute → feedback), but only one tool implementation (`fs.read`). The `shell.exec`, `patch.apply`, and `search.ripgrep` tool IDs exist as constants without implementations. The `ToolRegistry` is code-only with no provider abstraction, blocking future MCP integration.

## Goals

1. Implement three builtin tools: `shell.exec`, `patch.apply`, `search.ripgrep`
2. Introduce `ToolProvider` trait for unified tool discovery (builtin + MCP)
3. Integrate tools into the Agent Loop with proper definitions injection
4. Extend `ToolRisk` with `Destructive` level and update `PermissionEngine`

## Design Decisions

| Decision           | Choice                                 | Rationale                                                                                        |
| ------------------ | -------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Shell permissions  | Tiered: ReadOnly → Write → Destructive | Agents need to run builds/tests without asking, but dangerous operations always require approval |
| Patch format       | Unified diff                           | Compatible with `git diff` / `git apply`, most flexible for multi-line edits                     |
| Search strategy    | rg binary + fallback                   | Best performance when available, graceful degradation when not                                   |
| Tool extensibility | ToolProvider abstraction               | Builtin and MCP tools share the same registry/permission path; MCP provider is a placeholder     |

## Architecture

### ToolProvider Trait

```rust
#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn list_tools(&self) -> Vec<ToolDefinition>;
    async fn get_tool(&self, tool_id: &str) -> Option<Box<dyn Tool>>;
    fn name(&self) -> &str;
}
```

The `ToolRegistry` is refactored to hold a `Vec<Box<dyn ToolProvider>>` with an `RwLock<HashMap>` name-to-provider index cache. `add_provider` builds the cache; `get` does O(1) cache lookup then delegates to the provider. Provider priority is by registration order — first match wins. The existing `register(tool)` API is preserved by wrapping single tools in an anonymous provider.

### BuiltinProvider

Registers `ShellExecTool`, `RipgrepSearchTool`, and `PatchApplyTool` (plus existing `FsReadTool`) at construction. Created via `BuiltinProvider::with_defaults(workspace_root)`.

### McpProvider (Placeholder)

Skeleton with empty `list_tools()` and `get_tool()` returning `None`. Full implementation deferred to a future task.

## ShellExecTool

### Command Risk Classification

```rust
pub enum CommandRisk { ReadOnly, Write, Destructive, Unknown }
```

Classification rules:

- **ReadOnly**: ls, cat, head, tail, grep, rg, find, wc, sort, uniq, diff, echo, pwd, which, git (status/log/diff), cargo (test/build), npm (run/list), etc.
- **Write**: cp, mv, mkdir, touch, chmod, docker, kubectl, git (commit/merge), npm (install), pip (install), etc.
- **Destructive**: rm, sudo, su, mkfs, dd, format
- **Unknown**: anything not in the whitelist → conservative treatment

Subcommands can upgrade risk. For example, `git` defaults to ReadOnly, but `git commit` is Write and `git push --force` is Write (flagged for review).

### Permission Mapping

| CommandRisk | Readonly | Suggest          | Autonomous       |
| ----------- | -------- | ---------------- | ---------------- |
| ReadOnly    | Allowed  | Allowed          | Allowed          |
| Write       | Denied   | RequiresApproval | Allowed          |
| Destructive | Denied   | RequiresApproval | RequiresApproval |
| Unknown     | Denied   | RequiresApproval | RequiresApproval |

### Safety Hard Limits

1. **Working directory locked** to workspace_root via `current_dir()`
2. **Timeout**: default 30s, kills child process on expiry
3. **Output truncation** at `output_limit_bytes` (default 100KB)
4. **Environment sanitization**: remove sensitive vars (SSH_PRIVATE_KEY, etc.), preserve PATH/HOME/LANG

### Implementation

```rust
pub struct ShellExecTool {
    workspace_root: PathBuf,
    default_timeout: Duration,
    max_output_bytes: usize,
}
```

`risk()` parses the command string, classifies it, and maps `CommandRisk` to `ToolRisk`. `invoke()` runs `tokio::process::Command` with the classified safety constraints.

Command parsing is a simple whitespace split — `sh -c` wrapping is NOT used (avoiding shell injection). The program and args are passed directly to `Command::new(program).args(args)`.

## PatchApplyTool

### Input Format

Accepts unified diff strings compatible with `git diff` / `git apply`:

```json
{
  "patch": "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -10,3 +10,4 @@\n fn hello() {\n-    println!(\"hi\");\n+    println!(\"hello\");\n+    println!(\"world\");\n }\n"
}
```

Supports multi-file patches, new file creation (`--- /dev/null`), and file deletion (`+++ /dev/null`).

### Core Data Structures

```rust
pub struct FilePatch {
    pub old_path: PathBuf,
    pub new_path: PathBuf,
    pub hunks: Vec<Hunk>,
    pub is_new_file: bool,
    pub is_delete: bool,
}

pub struct Hunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<PatchLine>,
}

pub enum PatchLine {
    Context(String),
    Remove(String),
    Add(String),
}
```

### Parsing

Hand-written state machine (~150 lines) — no external crate dependency. Line-by-line: `--- a/` starts a new FilePatch, `+++ b/` sets new_path, `@@ -x,y +a,b @@` starts a Hunk, ` `/`-`/`+` prefixes become PatchLine variants.

### Application (Atomic)

Two-phase approach:

1. **Validate phase**: For every hunk in every file, verify that Context and Remove lines match the actual file content. No writes occur.
2. **Apply phase**: Only if all validations pass, write files. On any failure during apply, the design returns an error (since validation passed, apply should succeed; if it doesn't, it's a bug, not a user error).

New files: extract Add lines as content, create parent directories with `create_dir_all`, then write.
Deleted files: `remove_file`.
Modified files: splice hunk changes into the line array, rewrite the file.

### Workspace Security

All paths are resolved relative to workspace_root. Path escape (e.g., `../../etc/passwd`) is rejected.

### Risk Mapping

- New file creation or file deletion → `ToolRisk::destructive(PATCH_TOOL_ID)`
- Modification of existing files → `ToolRisk::write(PATCH_TOOL_ID)`

## RipgrepSearchTool

### Input

```json
{
  "pattern": "fn hello",
  "path": "src/",
  "file_glob": "*.rs",
  "max_results": 50
}
```

All fields except `pattern` are optional.

### Strategy

1. Try `rg` binary with `--json` output mode
2. If `rg` not found, fall back to built-in search

### rg Binary Path

```rust
fn find_rg_binary() -> Option<PathBuf>
```

Checks `KAIROX_RG_PATH` env var first, then `which rg` via `std::process::Command`.

### rg Engine

Invokes `rg --json --max-count --max-filesize 10M --sort-path --color never` with optional `--glob`. Parses each JSON line: `{"type":"match","data":{...}}` → `SearchResult`.

### Fallback Engine

Recursively walks the workspace (max depth 10, max 500 files), reads UTF-8 files, applies `regex::Regex` pattern matching. Skips hidden directories, `node_modules`, `target`, and binary files.

### Simple Glob Matching

Hand-written (~20 lines): supports `*.ext` and `*.{ext1,ext2}` patterns. No external `glob` crate.

### Output Format

```
[ripgrep] Found 3 matches in 2 files (50 max, not truncated):

src/main.rs:10:    fn hello() {
src/main.rs:15:    fn hello_world() {
src/lib.rs:42:    /// hello documentation
```

### Risk

Search is always `ToolRisk::read(SEARCH_TOOL_ID)`.

## Runtime Integration

### Current Problem

- `ModelRequest.tools` is always empty → model never knows available tools
- `LocalRuntime.tool_registry` is never populated with providers
- Tool results are concatenated as a single `[Tool results]` message

### Changes

1. `LocalRuntime::with_builtin_tools(workspace_root)` — registers BuiltinProvider
2. `LocalRuntime::with_provider(provider)` — registers custom providers
3. In `send_message()`: lock registry, call `list_all()`, inject into `ModelRequest.tools`
4. Tool results: each tool call result becomes a separate `add_tool_result()` message with `tool_call_id`, `tool_id`, and `result`

### ModelRequest Extension

```rust
impl ModelRequest {
    pub fn add_tool_result(self, tool_call_id: &str, tool_id: &str, result: &str) -> Self {
        self.messages.push(ModelMessage {
            role: "tool".into(),
            content: format!("tool_call_id={}\ntool_id={}\nresult={}", tool_call_id, tool_id, result),
        });
        self
    }
}
```

## Event Payload Extension

`ToolInvocationCompleted` gains additional fields:

```rust
EventPayload::ToolInvocationCompleted {
    invocation_id: String,
    output_preview: String,
    tool_id: String,           // NEW
    exit_code: Option<i32>,    // NEW (shell.exec only)
    duration_ms: u64,          // NEW
    truncated: bool,           // NEW
}
```

This is a breaking change to event serialization. Acceptable at v0.2.0 pre-stable.

## ToolRisk Extension

```rust
pub enum ToolRisk {
    Read(String),
    Write(String),
    Destructive(String),  // NEW
}
```

PermissionEngine updated:

| Risk        | Readonly | Suggest          | Autonomous       |
| ----------- | -------- | ---------------- | ---------------- |
| Read        | Allowed  | Allowed          | Allowed          |
| Write       | Denied   | RequiresApproval | Allowed          |
| Destructive | Denied   | RequiresApproval | RequiresApproval |

## Error Types

```rust
pub enum ToolError {
    NotFound(String),
    WorkspaceEscape(String),
    PermissionDenied(String),
    ExecutionFailed(String),
    OutputLimitExceeded(usize),
    Timeout(u64),
    PatchParseFailed(String),
    ContextMismatch { line: usize, expected: String, actual: String },
    RgNotFound,
    InvalidPattern(String),
    BinaryFile(String),
}
```

## File Structure

```
crates/agent-tools/src/
├── lib.rs                  # Updated module exports
├── permission.rs           # Extended: ToolRisk::Destructive, rules update
├── registry.rs             # Refactored: ToolProvider trait, ToolRegistry rewrite
├── provider/
│   ├── mod.rs              # ToolProvider trait, BuiltinProvider, McpProvider placeholder
│   ├── builtin.rs          # BuiltinProvider implementation
│   └── mcp.rs              # McpProvider skeleton (migrated from original mcp.rs)
├── shell.rs                # ShellExecTool + CommandRisk classification
├── search.rs               # RipgrepSearchTool + rg parsing + fallback
├── patch/
│   ├── mod.rs              # PatchApplyTool entry point
│   ├── parse.rs            # Unified diff parser
│   └── apply.rs            # Hunk application + atomicity
├── filesystem.rs           # Unchanged: FsReadTool
└── mcp.rs                  # Preserved: MCP type definitions
```

## Dependencies

- **`regex`** crate added to workspace — used by fallback search engine only
- No other new external crates: diff parser is hand-written, glob matching is hand-written, rg JSON parsing uses existing `serde_json`

## Implementation Order

| Step | Description                                            | Est. Lines | Parallelizable  |
| ---- | ------------------------------------------------------ | ---------- | --------------- |
| 1    | ToolRisk::Destructive + PermissionEngine update        | ~30        | No              |
| 2    | ToolProvider trait + ToolRegistry refactor             | ~150       | No              |
| 3    | PatchApplyTool (parse.rs + apply.rs + mod.rs)          | ~350       | Yes (with 4, 5) |
| 4    | ShellExecTool (shell.rs)                               | ~250       | Yes (with 3, 5) |
| 5    | RipgrepSearchTool (search.rs)                          | ~300       | Yes (with 3, 4) |
| 6    | BuiltinProvider + integration tests                    | ~100       | No              |
| 7    | LocalRuntime integration                               | ~80        | No              |
| 8    | Event payload extension + SessionProjection adaptation | ~50        | No              |

**Total estimate**: ~1,310 lines of new/modified code across 8 steps.

## Out of Scope

- Full MCP transport implementation (McpProvider is placeholder only)
- TUI/GUI integration of tool results display
- Tool result streaming (currently returns complete output)
- Sandboxed filesystem writes (relies on PermissionEngine gating)

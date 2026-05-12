# Refactor & Test Hardening — Design Spec

## Motivation

Kairox v0.19.0 has grown rapidly through 34 design specs. All 963 tests pass and there are no open issues, but two structural concerns need attention:

1. **Large files** — `facade_runtime.rs` (2758 lines), `agent_loop.rs` (1496 lines), and the `AppFacade` trait (1263 lines, ~60 methods) have absorbed too many responsibilities, making them hard to reason about and test in isolation.
2. **Test gaps** — `agent-models`, `agent-tools`, `agent-store`, and `agent-config` have zero integration tests, relying entirely on unit tests and the downstream runtime/full-stack tests for coverage.

This spec defines a balanced program: add integration tests for the under-tested crates, then decompose the large files into focused modules, then split the monolithic facade trait.

## Guiding principles

- **Every step is independently committable and revertible.**
- **`cargo test --workspace` stays green throughout.**
- **No public API changes** — all method signatures and IPC surfaces remain identical. CI type-sync (`just check-types`) must pass after every step.
- **Each extraction carries its own tests** — tests move from the old file to the new module alongside the code they test.

## Phase 1: Test foundation

Add integration tests for crates that currently lack them. These tests run against real (in-memory SQLite, real filesystem, real HTTP) or deterministic fakes and serve as a safety net for the refactoring phases.

### 1.1 `agent-store` integration tests

**Why first**: lowest in the dependency stack; everything depends on it.

- Event persistence round-trip: write `DomainEvent`, read back via `EventStore::query`
- Metadata CRUD: create workspace → create session → rename → soft-delete → cleanup
- Concurrent writes: two tasks writing events to the same session simultaneously
- `ProjectMetaRepository`: create, list, reorder, remove projects

File: `crates/agent-store/tests/integration.rs`

### 1.2 `agent-config` integration tests

- TOML parsing round-trip: `kairox.toml` → `Config` → profile resolution → `ModelRouter`
- API key resolution: `api_key_env` referencing real env vars
- Profile discovery: `build_router` with multiple profiles, verify correct client is selected
- `.kairox/` discovery: project-local config override of global config

File: `crates/agent-config/tests/integration.rs`

### 1.3 `agent-models` integration tests

- `ModelRouter` selection: multiple profiles, verify routing by profile name
- Model switching: change profile mid-session, verify new client is used
- Provider-specific: smoke test each provider type (OpenAI-compatible, Anthropic, Ollama, Fake)

File: `crates/agent-models/tests/integration.rs`

### 1.4 `agent-tools` integration tests

- Full tool pipeline: `BuiltinProvider` → `ToolRegistry` → tool lookup + execution
- Permission engine: each `PermissionMode` variant through decide → execute path
- Shell tool: execute a trivial shell command, verify stdout capture
- FS tools: read/write/list against a tempdir, verify content and error on out-of-bounds paths
- MCP tool adapter: register an MCP tool in the registry, verify it's discoverable and executable

File: `crates/agent-tools/tests/integration.rs`

## Phase 2: Decompose facade_runtime.rs

`facade_runtime.rs` is the largest file in the codebase. Its `impl AppFacade for LocalRuntime` block (lines 774–2331, ~1557 lines) contains ~60 methods spanning five distinct functional domains. We extract each domain into its own file while keeping the struct definition and builder in `facade_runtime.rs`.

### 2.1 Extract session management → `facade_sessions.rs`

Methods moved:

- `open_workspace`, `start_session`
- `cancel_session`, `get_session_projection`, `get_trace`
- `list_workspaces`, `list_sessions`, `rename_session`, `soft_delete_session`, `cleanup_expired_sessions`
- `mark_session_visible` (internal helper)

### 2.2 Extract skills management → `facade_skills.rs`

Methods moved:

- `list_skills`, `get_skill`, `activate_skill`, `deactivate_skill`, `list_active_skills`
- `list_skill_settings`, `get_skill_settings_detail`, `set_skill_enabled`, `delete_skill_settings`
- `search_remote_skills`, `install_remote_skill`, `install_github_skill`, `update_skill`
- `list_skill_catalog`, `list_skill_sources`, `add_skill_source`, `remove_skill_source`, `set_skill_source_enabled`, `refresh_skill_catalog`

### 2.3 Extract MCP management → `facade_mcp.rs`

Methods moved:

- `list_mcp_server_settings`, `upsert_mcp_server_settings`, `delete_mcp_server_settings`, `set_mcp_server_enabled`, `open_mcp_config_file`
- `list_catalog`, `get_catalog_entry`, `refresh_catalog`, `install_catalog_entry`, `uninstall_catalog_entry`, `list_installed_entries`
- `list_catalog_sources`, `add_catalog_source`, `remove_catalog_source`, `set_catalog_source_enabled`

### 2.4 Extract project management → `facade_projects.rs`

Methods moved:

- `list_projects`, `create_blank_project`, `add_existing_project`, `rename_project`, `remove_project`
- `restore_project_session`, `update_project_order`, `update_project_expanded`
- `create_project_draft_session`, `create_project_worktree_session`

### 2.5 Slim facade_runtime.rs to a thin delegation layer

After extraction, the `AppFacade` impl becomes ~200 lines of delegation:

```rust
#[async_trait]
impl<S, M> AppFacade for LocalRuntime<S, M> {
    async fn open_workspace(&self, path: String) -> Result<WorkspaceInfo> {
        self.open_workspace_impl(path).await
    }
    // ... each method delegates to an extracted impl
}
```

The core methods that stay in `facade_runtime.rs`:

- `send_message` (orchestrates multiple subsystems)
- `decide_permission`
- `resolve_permission`, `compact_session`, `switch_model` (runtime operations)
- Builder methods
- Test helpers

Target: `facade_runtime.rs` shrinks from 2758 → ~600 lines (struct + builder + core ops + delegation + tests).

## Phase 3: Decompose agent_loop.rs

`agent_loop.rs` (1496 lines) contains the main agent loop with interleaved concerns: context assembly, budget checking, tool execution, and response processing. Extract into submodules under `agent_loop/`.

### 3.1 Extract `agent_loop/budget.rs`

- Context budget checking (`within_budget`, `should_trigger_auto_compaction`)
- Budget configuration helpers

### 3.2 Extract `agent_loop/messages.rs`

- `build_model_messages` — assembles messages for the model
- Compaction summary substitution (replaces truncated event ranges with summary markers)
- System prompt construction

### 3.3 Extract `agent_loop/tool_loop.rs`

- Tool call execution loop: send → receive tool calls → execute → collect results → resend
- Tool result formatting and error handling

### 3.4 `agent_loop/runner.rs`

- `run_agent_loop` — top-level orchestrator that calls into the other modules
- Entry point coordination

Target: `agent_loop.rs` replaced by `agent_loop/` directory with 4 focused files.

## Phase 4: Split the AppFacade trait

After Phase 2, the impl is already delegation-based. Now split the trait definition itself so consumers can depend on only the facet they need.

### 4.1 Define sub-traits in `agent-core/src/facade/`

```
agent-core/src/facade/
├── mod.rs              # AppFacade = SessionFacade + SkillsFacade + McpFacade + ProjectFacade
├── session.rs          # SessionFacade — session lifecycle, tracing, workspace listing
├── skills.rs           # SkillsFacade — skill activation, settings, marketplace
├── mcp.rs              # McpFacade — MCP server settings, catalog, installation
└── project.rs          # ProjectFacade — project management, workspace sessions
```

### 4.2 Update GUI/TUI

Places that only need session methods (e.g., trace panel, session sidebar) can use `Arc<dyn SessionFacade>` instead of the full `Arc<dyn AppFacade>`. This is a gradual migration — the full `AppFacade` supertrait remains available everywhere.

### 4.3 Backward compatibility

`AppFacade` remains as a supertrait combining all sub-traits. Existing code using `Arc<dyn AppFacade>` compiles without changes. The sub-traits are an additive decomposition.

## Execution order

```
Phase 1 (test foundation)
├── 1.1 agent-store integration tests
├── 1.2 agent-config integration tests
├── 1.3 agent-models integration tests
└── 1.4 agent-tools integration tests

Phase 2 (decompose facade_runtime.rs)
├── 2.1 Extract facade_sessions.rs
├── 2.2 Extract facade_skills.rs
├── 2.3 Extract facade_mcp.rs
├── 2.4 Extract facade_projects.rs
└── 2.5 Slim facade_runtime.rs to delegation layer

Phase 3 (decompose agent_loop.rs)
├── 3.1 Extract messages.rs + budget.rs
├── 3.2 Extract tool_loop.rs
└── 3.3 Extract runner.rs

Phase 4 (split AppFacade trait)
├── 4.1 Define SessionFacade + SkillsFacade + McpFacade + ProjectFacade
├── 4.2 AppFacade becomes trait alias / supertrait
└── 4.3 Update consumers to use sub-traits where appropriate
```

## Risk mitigation

- **Each step is a separate commit.** If a step causes issues, only that commit is reverted.
- **`cargo test --workspace` must pass after each step.** No batch steps that leave tests broken.
- **`just check-types` must pass after each step that touches facade methods.** TypeScript bindings must stay in sync.
- **Phases 1–3 are non-breaking.** Only Phase 4 changes the trait hierarchy; it's additive (new sub-traits, supertrait stays).
- **No behavior changes.** All refactoring is structural — the runtime behaves identically before and after.

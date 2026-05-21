# TUI MCP Marketplace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add MCP marketplace and settings management to the TUI MCP overlay while keeping the change scoped to MCP.

**Architecture:** Expand the existing `McpOverlay` from a runtime-only list into a tabbed, snapshot-driven overlay. The app builds a single MCP snapshot from existing `AppFacade` MCP methods plus existing runtime-manager methods; the overlay owns selection state and emits typed `Command` variants for mutations.

**Tech Stack:** Rust, ratatui, crossterm, `agent_core::facade::McpFacade`, `agent_runtime::LocalRuntime`, existing TUI component tests and app command tests.

---

### Task 1: Overlay State And Commands

**Files:**

- Modify: `crates/agent-tui/src/components/mod.rs`
- Modify: `crates/agent-tui/src/components/mcp_overlay.rs`

- [x] **Step 1: Write failing overlay selection tests**

Add tests proving the overlay can switch between Runtime, Settings, Catalog, and Sources tabs, preserves per-tab selection, and emits these MCP-specific commands:

```rust
Command::SetMcpServerEnabled { server_id, enabled }
Command::DeleteMcpServerSettings { server_id }
Command::InstallMcpServer { request }
Command::UninstallMcpServer { server_id }
Command::SetMcpCatalogSourceEnabled { source_id, enabled }
```

Run: `cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test -p agent-tui mcp_overlay`
Expected: FAIL because the snapshot, tab state, and command variants do not exist yet.

- [x] **Step 2: Add snapshot DTOs and command variants**

Add `McpOverlaySnapshot` to `components/mod.rs` with:

```rust
pub struct McpOverlaySnapshot {
    pub runtime_servers: Vec<McpServerEntry>,
    pub settings: Vec<agent_core::facade::McpServerSettingsView>,
    pub installed: Vec<agent_core::facade::InstalledEntry>,
    pub catalog: Vec<agent_core::facade::ServerEntry>,
    pub sources: Vec<agent_core::facade::CatalogSourceView>,
}
```

Change `CrossPanelEffect::ShowMcpOverlay` to carry `McpOverlaySnapshot`, and add command variants for settings/catalog/source actions.

- [x] **Step 3: Implement tabbed overlay state**

Replace the single server list state with per-tab `ListState`s and a local tab enum. Render compact row lines for:

- Runtime: status, server id, tool count, trust.
- Settings: enabled state, id/name, runtime status, source, writable.
- Installed: running state, server id, catalog/source.
- Catalog: display name, source, trust, summary.
- Sources: enabled state, id/display name, kind, URL or builtin.

- [x] **Step 4: Run overlay tests green**

Run: `cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test -p agent-tui mcp_overlay`
Expected: PASS.

### Task 2: Facade-backed App Commands

**Files:**

- Modify: `crates/agent-tui/src/app/commands.rs`
- Modify: `crates/agent-tui/tests/app_logic.rs`

- [x] **Step 1: Write failing fake-facade tests**

Add a test facade implementing `McpFacade` that records calls and returns deterministic settings, catalog, installed entries, and sources. Test that:

- `Command::OpenMcpOverlay` calls list settings, list installed entries, list catalog, and list catalog sources.
- `Command::SetMcpServerEnabled` calls `set_mcp_server_enabled` and refreshes the overlay.
- `Command::InstallMcpServer` calls `install_catalog_entry` with the selected `InstallRequest`.
- `Command::UninstallMcpServer` calls `uninstall_catalog_entry`.
- `Command::SetMcpCatalogSourceEnabled` calls `set_catalog_source_enabled`.

Run: `cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test -p agent-tui tui_mcp`
Expected: FAIL because these commands are not handled by `app::dispatch_commands`.

- [x] **Step 2: Implement generic MCP command handling**

In `app/commands.rs`, import `McpFacade`, handle the new MCP settings/catalog/source command variants, and build the non-runtime portion of the overlay snapshot from facade methods.

- [x] **Step 3: Keep runtime server actions in main dispatch**

In `main.rs`, continue using the existing runtime MCP manager for trust/start/stop/refresh. Replace the old runtime-only refresh helper with a helper that asks `app::refresh_mcp_overlay` for facade data and then overlays the runtime server list.

- [x] **Step 4: Run app command tests green**

Run: `cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test -p agent-tui tui_mcp`
Expected: PASS.

### Task 3: Focused Verification

**Files:**

- Verify only.

- [x] **Step 1: Run focused MCP tests**

Run: `cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test -p agent-tui mcp`
Expected: PASS.

- [x] **Step 2: Run TUI compile check**

Run: `cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' check -p agent-tui`
Expected: PASS.

- [x] **Step 3: Run required repo gates**

Run:

```bash
bun run format:check
bun run lint:web
cargo --config 'source.ustc.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test --workspace --all-targets
```

Expected: PASS. The default `ustc` sparse cache initially lagged new lockfile crates, then passed after Cargo refreshed the source cache.

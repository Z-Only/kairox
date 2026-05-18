# Plugin System Marketplace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a safe first version of Kairox plugin discovery, settings, and marketplace installation.

**Architecture:** Add an `agent-plugins` crate for manifest/state/marketplace parsing, expose plugin DTOs through `agent-core`, wire runtime facade methods in `agent-runtime`, and add Tauri commands plus a Vue Settings tab. Plugin skills are discovered as namespaced skill roots; executable plugin components are inventory-only.

**Tech Stack:** Rust, Tokio, serde, toml, tauri-specta, Vue 3, Pinia, Vitest, Playwright, tauri-mock.

---

### Task 1: Core Plugin Crate

**Files:**

- Create: `crates/agent-plugins/Cargo.toml`
- Create: `crates/agent-plugins/src/lib.rs`
- Create: `crates/agent-plugins/src/manifest.rs`
- Create: `crates/agent-plugins/src/settings.rs`
- Modify: `Cargo.toml`

- [x] Write tests for parsing `.codex-plugin/plugin.json` and `.claude-plugin/plugin.json`.
- [x] Implement `PluginManifest`, `PluginComponentInventory`, and manifest resolution order.
- [x] Write tests for user/project/builtin precedence and invalid manifest visibility.
- [x] Implement settings discovery and `plugins-state.toml` persistence.
- [x] Run `cargo test -p agent-plugins` and verify the new tests fail before implementation and pass after implementation.

### Task 2: Runtime And DTOs

**Files:**

- Modify: `crates/agent-core/src/facade.rs`
- Create: `crates/agent-core/src/facade/plugins.rs`
- Modify: `crates/agent-core/src/lib.rs`
- Modify: `crates/agent-runtime/src/lib.rs`
- Create: `crates/agent-runtime/src/plugin_settings.rs`
- Create: `crates/agent-runtime/src/facade_plugins.rs`
- Modify: `crates/agent-runtime/src/skills.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`

- [x] Write facade/runtime tests for listing plugins and toggling enabled state.
- [x] Add plugin DTOs: settings view, detail view, source view, catalog entry, install request.
- [x] Add runtime roots: `~/.config/kairox/plugins`, `<workspace>/.kairox/plugins`, optional built-in root.
- [ ] Append plugin skill roots to skill discovery with namespaced plugin skill IDs.
- [x] Run `cargo test -p agent-runtime plugin` and `cargo test -p agent-core`.

### Task 3: Marketplace Support

**Files:**

- Create: `crates/agent-plugins/src/marketplace.rs`
- Create: `crates/agent-runtime/src/plugin_sources_toml.rs`
- Modify: `crates/agent-runtime/src/facade_plugins.rs`

- [x] Write tests for Claude-style `marketplace.json` parsing.
- [x] Support source kinds: URL, GitHub shorthand, and local file/path for first version.
- [x] Implement install by copying a resolved local plugin directory into user/project plugin roots.
- [x] For remote sources, fetch marketplace JSON and expose catalog entries; defer package download to explicit install.
- [x] Run `cargo test -p agent-plugins marketplace`.

### Task 4: Tauri Commands And Generated Types

**Files:**

- Create: `apps/agent-gui/src-tauri/src/commands/plugins.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify generated: `apps/agent-gui/src/generated/commands.ts`

- [x] Add commands for list/detail/set-enabled/delete/list-sources/set-source-enabled/list-catalog/install.
- [x] Add command tests for request/response serialization.
- [x] Run `cargo test -p agent-gui-tauri plugins`.
- [x] Run Specta export and inspect generated command diffs.

### Task 5: GUI Store And Settings Tab

**Files:**

- Create: `apps/agent-gui/src/stores/plugins.ts`
- Create: `apps/agent-gui/src/components/PluginSettingsPane.vue`
- Modify: `apps/agent-gui/src/layouts/SettingsLayout.vue`
- Modify: `apps/agent-gui/src/router/routes.ts`
- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`

- [ ] Write a focused Vitest test for the plugin store command flow.
- [x] Add Settings `Plugins` tab and show `ConfigSourceBar` for it.
- [x] Implement Installed, Discover, and Marketplaces subtabs.
- [x] Keep controls dense and settings-like; do not add marketing layout.
- [x] Run the full GUI Vitest suite.

### Task 6: E2E And Tauri Mock

**Files:**

- Modify: `apps/agent-gui/e2e/helpers/tauriMock.ts`
- Create: `apps/agent-gui/e2e/plugins-settings.spec.ts`

- [x] Add mock command handlers and plugin fixtures.
- [x] Test installed plugin rows, user/project source switching, enable/disable, marketplace source listing, and install from catalog.
- [x] Run focused Playwright plugin settings spec.
- [ ] Run `tauri-pilot` for the Plugins settings workflow if desktop environment is available; otherwise document the blocker and keep Playwright fallback.

### Task 7: Final Verification

**Files:**

- All changed files

- [x] Run `cargo test -p agent-plugins`.
- [x] Run `cargo test -p agent-runtime plugin`.
- [x] Run `cargo test -p agent-gui-tauri`.
- [x] Run Specta export and ensure generated diffs are intentional.
- [x] Run `bun run lint:web`.
- [x] Run focused Playwright plugin settings spec.
- [x] Run broader required checks from Kairox workflow after rebase.

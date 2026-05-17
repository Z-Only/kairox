# Hooks Config UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add user and project scoped hook configuration, built-in hook templates, a runtime command hook executor, and a dedicated GUI settings page.

**Architecture:** Parse hooks from `.kairox/config.toml` into `agent_config::Config`, keep GUI read/write helpers in `agent_runtime::hooks_settings`, and run command hooks from `agent_runtime::hooks` at lifecycle points in the agent loop. The Vue settings pane edits only the selected config layer and preserves existing TOML content through `toml_edit`.

**Tech Stack:** Rust, Tokio, toml/toml_edit, Tauri Specta, Vue 3, TypeScript, Vitest.

---

### Task 1: Config and Settings Model

**Files:**

- Modify: `crates/agent-config/src/lib.rs`
- Modify: `crates/agent-config/src/loader.rs`
- Create: `crates/agent-runtime/src/hooks_settings.rs`
- Modify: `crates/agent-runtime/src/lib.rs`
- Modify: `crates/agent-core/src/facade/settings.rs`

- [ ] Add failing Rust tests for TOML hooks parsing and preserving config edits.
- [ ] Implement hook DTOs in `agent_config`.
- [ ] Implement `hooks_settings` read/write helpers and built-in templates.
- [ ] Add facade DTOs for the GUI.
- [ ] Run focused Rust tests.

### Task 2: Runtime Hook Executor

**Files:**

- Create: `crates/agent-runtime/src/hooks.rs`
- Modify: `crates/agent-runtime/src/agent_loop/runner.rs`
- Modify: `crates/agent-runtime/src/agent_loop/tool_loop.rs`
- Modify: `crates/agent-runtime/src/facade_runtime.rs`

- [ ] Add failing tests for matcher selection and command hook execution.
- [ ] Implement command execution with timeout, JSON stdin payload, and non-fatal error logging.
- [ ] Trigger `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, and `Stop`; expose `SessionStart` support for future session wiring.
- [ ] Run focused runtime tests.

### Task 3: Tauri Commands and Generated Types

**Files:**

- Create: `apps/agent-gui/src-tauri/src/commands/settings/hooks.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands/settings/mod.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Modify: `apps/agent-gui/src-tauri/src/bin/export_specta.rs`
- Regenerate: `apps/agent-gui/src/generated/commands.ts`

- [ ] Add commands to list, upsert, delete, and open hook config files.
- [ ] Register commands and Specta types.
- [ ] Run `cargo test -p agent-gui-tauri`.
- [ ] Run `just gen-types` and inspect generated diffs.

### Task 4: GUI Settings Pane

**Files:**

- Create: `apps/agent-gui/src/components/HooksSettingsPane.vue`
- Create: `apps/agent-gui/src/components/HooksSettingsPane.test.ts`
- Modify: `apps/agent-gui/src/router/routes.ts`
- Modify: `apps/agent-gui/src/layouts/SettingsLayout.vue`
- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`

- [ ] Add failing Vitest coverage for load, save, delete, scope switching, and template insertion.
- [ ] Implement hooks tab UI with event grouping, hook form, and template actions.
- [ ] Keep source selector behavior aligned with existing MCP/skills/models/instructions panes.
- [ ] Run focused Vitest.

### Task 5: Verification and PR

- [ ] Run `cargo test -p agent-config`.
- [ ] Run `cargo test -p agent-runtime`.
- [ ] Run `cargo test -p agent-gui-tauri`.
- [ ] Run focused GUI Vitest.
- [ ] Run required GUI verification for settings behavior.
- [ ] Run broader lint/build/test checks appropriate to the final diff.
- [ ] Commit, rebase on `origin/main`, push, open PR, enable squash auto-merge, watch CI, merge, and clean up per `kairox-dev-workflow`.

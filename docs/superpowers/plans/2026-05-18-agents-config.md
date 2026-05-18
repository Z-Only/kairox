# Agents Config Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build user/project/built-in specialized agent configuration with a dedicated GUI settings tab.

**Architecture:** Add a focused runtime settings module that discovers and mutates Markdown agent definition files, expose typed DTOs through `agent-core` facade traits and Tauri commands, then add a Pinia store and Vue settings pane. Runtime scheduling remains unchanged in this pass.

**Tech Stack:** Rust, serde_yaml, tokio fs, Tauri commands, Specta-generated TypeScript bindings, Vue 3, Pinia, Vitest.

---

### Task 1: Backend Agent Settings

**Files:**

- Create: `crates/agent-runtime/src/agent_settings.rs`
- Modify: `crates/agent-runtime/src/lib.rs`
- Modify: `crates/agent-runtime/Cargo.toml`
- Modify: `crates/agent-core/src/facade/settings.rs`
- Modify: `crates/agent-core/src/facade.rs`

- [x] Write failing Rust tests in `agent_settings.rs` for parsing, invalid names, built-in/user/project precedence, upsert, and delete.
- [x] Run `cargo test -p agent-runtime agent_settings -- --nocapture` and confirm the new tests fail because the module does not exist.
- [x] Implement parser, built-in definitions, discovery, upsert, copy, and delete helpers.
- [x] Add facade DTOs: `AgentSettingsInput`, `AgentSettingsView`, `EffectiveAgentView`, and `AgentSettingsScope`.
- [x] Run `cargo test -p agent-runtime agent_settings -- --nocapture` and confirm the tests pass.

### Task 2: Runtime Facade And Tauri Commands

**Files:**

- Modify: `crates/agent-core/src/facade.rs`
- Create: `crates/agent-core/src/facade/agents.rs`
- Modify: `crates/agent-runtime/src/facade_runtime.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands/settings/mod.rs`
- Create: `apps/agent-gui/src-tauri/src/commands/settings/agents.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`

- [x] Add failing facade-level or command-level tests where practical; otherwise rely on runtime unit tests and generated binding compilation.
- [x] Add `AgentsFacade` methods for listing, upserting, deleting, and copying agents.
- [x] Add runtime roots: user `~/.config/kairox/agents`, project `.kairox/agents`, built-in in code.
- [x] Register Tauri commands and Specta exports.
- [x] Run `cargo test -p agent-gui-tauri` and fix compile/test failures.

### Task 3: GUI Store And Settings Tab

**Files:**

- Create: `apps/agent-gui/src/stores/agentSettings.ts`
- Create: `apps/agent-gui/src/components/AgentSettingsPane.vue`
- Create: `apps/agent-gui/src/components/AgentSettingsPane.test.ts`
- Modify: `apps/agent-gui/src/router/routes.ts`
- Modify: `apps/agent-gui/src/layouts/SettingsLayout.vue`
- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`

- [x] Write failing Vitest tests for rendering agents, switching scope, editing an agent, and deleting a writable agent.
- [x] Run the focused Vitest command and confirm tests fail because the component/store does not exist.
- [x] Implement Pinia store wrappers around generated commands.
- [x] Implement the settings tab UI using existing settings page patterns.
- [x] Run the focused Vitest command and confirm tests pass.

### Task 4: Generated Types And Verification

**Files:**

- Modify: `apps/agent-gui/src/generated/commands.ts`
- Modify: `apps/agent-gui/src/generated/events.ts` only if Specta changes require it.

- [x] Regenerate Specta TypeScript bindings.
- [x] Run `cargo test -p agent-runtime agent_settings -- --nocapture`.
- [x] Run `cargo test -p agent-gui-tauri`.
- [x] Run focused GUI tests for `AgentSettingsPane`.
- [x] Run required GUI gate: focused Playwright or Tauri Pilot for settings navigation, with Tauri Pilot preferred if the desktop environment supports it.
- [x] Run broader checks selected from the Kairox workflow before PR creation.

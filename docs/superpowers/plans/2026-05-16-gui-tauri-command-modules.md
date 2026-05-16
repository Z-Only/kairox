# GUI Tauri Command Modules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the large GUI Tauri `commands.rs` file into focused command modules while preserving all exported IPC command names and generated Specta bindings.

**Architecture:** Keep `apps/agent-gui/src-tauri/src/commands.rs` as the public facade for `crate::commands::*`. Move implementation into `commands/{session,project,settings,marketplace,skills,chat,draft}.rs`, while keeping shared DTOs and cross-module helpers in the facade so existing `crate::commands::<type_or_command>` paths remain stable.

**Tech Stack:** Rust, Tauri 2 command macros, tauri-specta, existing `agent-core` facade traits.

---

### Task 1: Preserve Public Command Surface

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`

- [ ] **Step 1: Keep shared response/request types in the facade**

Keep every shared `#[derive(..., specta::Type)]` DTO in `commands.rs`, preserving field names, serde attributes, and Specta annotations unchanged.

- [ ] **Step 2: Keep cross-module helper functions in the facade**

Keep cross-module helpers such as session/project conversion helpers in `commands.rs`. Keep domain-local helpers, such as attachment enrichment, catalog conversions, system file opener, and workspace file walking, beside their command modules.

- [ ] **Step 3: Re-export submodule commands**

Replace the old monolithic `commands.rs` body with module declarations and `pub use` statements so `crate::commands::<command_name>` remains valid for `lib.rs`, `specta.rs`, and `bin/export_specta.rs`.

### Task 2: Split Commands By Domain

**Files:**

- Create: `apps/agent-gui/src-tauri/src/commands/chat.rs`
- Create: `apps/agent-gui/src-tauri/src/commands/session.rs`
- Create: `apps/agent-gui/src-tauri/src/commands/project.rs`
- Create: `apps/agent-gui/src-tauri/src/commands/settings.rs`
- Create: `apps/agent-gui/src-tauri/src/commands/skills.rs`
- Create: `apps/agent-gui/src-tauri/src/commands/marketplace.rs`
- Create: `apps/agent-gui/src-tauri/src/commands/draft.rs`

- [ ] **Step 1: Put chat commands in `chat.rs`**

Move `initialize_workspace`, `start_session`, and `send_message`, plus attachment handling helpers if they are not shared elsewhere.

- [ ] **Step 2: Put session commands in `session.rs`**

Move session, memory, permission, task graph, model switching, build info, and workspace restore commands.

- [ ] **Step 3: Put project commands in `project.rs`**

Move project CRUD, project session listing, git status, worktree session, instruction summary, and workspace file listing commands.

- [ ] **Step 4: Put settings commands in `settings.rs`**

Move profile/config commands, profile settings, MCP settings/runtime commands, connectivity tests, and config-directory opener commands.

- [ ] **Step 5: Put skills commands in `skills.rs`**

Move active skill commands, skill settings, remote install/update commands, and skill catalog source commands.

- [ ] **Step 6: Put marketplace commands in `marketplace.rs`**

Move MCP marketplace catalog query/install/source commands and associated catalog conversion helpers.

- [ ] **Step 7: Put draft commands in `draft.rs`**

Move `save_draft` and `get_draft`.

### Task 3: Verify No IPC Contract Drift

**Files:**

- Modify if needed: `apps/agent-gui/src-tauri/src/specta.rs`
- Modify if needed: `apps/agent-gui/src-tauri/src/bin/export_specta.rs`
- Modify if needed: `apps/agent-gui/src-tauri/src/lib.rs`

- [ ] **Step 1: Run formatting**

Run: `cargo fmt --all`

- [ ] **Step 2: Regenerate TypeScript bindings**

Run: `just gen-types`

- [ ] **Step 3: Check generated diff**

Run: `git diff -- apps/agent-gui/src/generated/commands.ts apps/agent-gui/src/generated/events.ts`

Expected: no diff, unless command ordering changes force a harmless generated reorder.

- [ ] **Step 4: Run focused Tauri tests**

Run: `cargo test -p agent-gui-tauri`

Expected: command compiles and tests pass.

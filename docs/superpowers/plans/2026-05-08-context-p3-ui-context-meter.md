# Context P3 — GUI/TUI Context Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface live context-window usage and the manual-compaction control to both interfaces — a `ContextMeter.vue` segmented bar with an interactive popover at the top of the GUI's `ChatPanel`, and a single status-bar line in the TUI showing `profile · perm · ctx X/Y [bar] sys/tools/mem/hist/tres`. Both UIs react to the four `ContextCompaction*` events shipped in P2 and expose a `compact_session` action.

**Architecture:** P1 already emits `EventPayload::ContextAssembled { usage }` (carrying a full `ContextUsage` with per-source breakdown) and P2 already exposes `LocalRuntime::compact_session(SessionId, CompactionReason)` plus the four compaction events. P3 is a pure observability + control-surface layer — no runtime semantics change. (1) `agent-core` projection grows three new fields (`last_context_usage: Option<ContextUsage>`, `model_limits: Option<ProjectedModelLimits>`, `compaction: CompactionStatus`) so historical playback survives. (2) Two new Tauri commands wrap the existing facade methods (`compact_session`, `list_profiles_with_limits`). (3) A new `ContextMeter.vue` renders the bar + popover; `useSessionStore` grows reactive fields driven by `useTauriEvents`. (4) The existing TUI `crates/agent-tui/src/components/status_bar.rs` (342 lines, holds `info: StatusInfo`) is extended: `StatusInfo` gains two fields (`context_usage`, `compacting`), and a new `render_context_line_string` is invoked from `render_status_bar` when usage is present. A new `:compact` slash-command is intercepted in `ChatPanel::apply_key_action`, emitted as `Command::CompactSession`, and dispatched in `main.rs::dispatch_commands` by awaiting `runtime.compact_session(session_id, CompactionReason::UserRequested)`.

**Tech Stack:** Rust 1.x · Vue 3 (Composition API + `<script setup lang="ts">`) · Pinia (setup-stores) · vue-i18n · ratatui · tauri-specta · Vitest · `@vue/test-utils` · Playwright · `cargo test --workspace --all-targets` · `cargo clippy --workspace --all-targets --all-features -- -D warnings` · `pnpm run lint` · `just gen-types` · `just check-types`

**Branch:** `feat/context-p3-ui-context-meter` (already created via `git worktree add .worktrees/feat-context-p3-ui-context-meter -b feat/context-p3-ui-context-meter main`).

**Spec reference:** `docs/superpowers/specs/2026-05-08-session-context-and-model-management-design.md` §4.6 / §5

**Out of scope (deferred to P4):**

- `switch_model` Tauri command + the **Switch model…** dropdown in the popover (the popover button is rendered but DISABLED with a `data-test="switch-model-disabled"` marker; clicking is a no-op until P4).
- `ModelProfileSwitched` event handling (P4 adds the event variant; the projection field shape we add is forward-compatible).
- TUI `:model <alias>` command (P4).
- Per-MCP-server pinning, tokeniser swap, or "edit summary" UI.

---

## File Structure

> See task list below for which task touches each file. Every file path is exact.

### `agent-core` — projection fields + tests

- **Modify** `crates/agent-core/src/projection.rs`
  - Add three optional fields to `SessionProjection`:
    - `last_context_usage: Option<ContextUsage>` — replaced wholesale on each `ContextAssembled`.
    - `model_limits: Option<ModelLimits>` — set on `SessionInitialized` (resolved in a follow-up task; for P3 we project from `ContextAssembled.usage.{context_window, output_reservation}` which is enough to render the meter).
    - `compaction: CompactionStatus` — `Idle | Running | Failed { error: String }`, defaulting to `Idle`.
  - Add the new `apply` arms for `ContextAssembled` / `ContextCompactionStarted` / `ContextCompactionCompleted` / `ContextCompactionFailed` (the latter three flip `compaction` only — `CompactionSummary` stays in the catch-all because it's purely a model-context concern, not a UI projection one).
  - Re-export the new enum from `agent_core::projection`.

### `agent-core` — `CompactionStatus` enum

- **Modify** `crates/agent-core/src/lib.rs`
  - Re-export `CompactionStatus`.
- The enum itself lives next to `SessionProjection` in `projection.rs` (it's a UI projection concept, not a runtime state). It needs `serde` + `#[cfg_attr(feature = "specta", derive(specta::Type))]` because the GUI consumes it through the projection JSON.

### Tauri commands + specta

- **Modify** `apps/agent-gui/src-tauri/src/commands.rs`
  - Add `compact_session(state) -> Result<(), String>` — looks up the current `SessionId` and `WorkspaceId` from `GuiState` and forwards to `LocalRuntime::compact_session(_, CompactionReason::UserRequested)`.
  - Add `list_profiles_with_limits(state) -> Vec<ProfileWithLimits>` — iterates `state.config.profile_names()`, looks up each via `agent_config::resolve_limits`, returns `{ alias, provider, model_id, limits, has_api_key }`.
  - New DTOs: `ProfileWithLimits` (already-derives `serde::Serialize, Deserialize, specta::Type`).
- **Modify** `apps/agent-gui/src-tauri/src/lib.rs`
  - Register both commands in `generate_handler![...]`.
- **Modify** `apps/agent-gui/src-tauri/src/specta.rs`
  - Add both commands to `collect_commands![...]`.
  - Register `ProfileWithLimits` and `CompactionStatus` via `.typ::<>()`.
- **Modify** `crates/agent-runtime/src/facade_runtime.rs` — only if needed: confirm `compact_session` already takes `(SessionId, CompactionReason)` and is `pub`. (P2 added it; P3 verifies the signature, no changes expected.)

### Generated types

- **Auto-regenerate** `apps/agent-gui/src/generated/commands.ts` and `apps/agent-gui/src/generated/events.ts` via `just gen-types`. Verified by `just check-types` in the final task.

### GUI — Pinia store + composables

- **Modify** `apps/agent-gui/src/stores/session.ts`
  - Add reactive state: `lastContextUsage: Ref<ContextUsage | null>`, `modelLimits: Ref<ModelLimits | null>`, `compacting: Ref<boolean>`, `lastCompactionError: Ref<string | null>`.
  - Extend `applyEvent`'s `switch (p.type)` with cases for `ContextAssembled` / `ContextCompactionStarted` / `ContextCompactionCompleted` / `ContextCompactionFailed`.
  - In `setProjection`, hydrate `lastContextUsage` / `modelLimits` / `compacting` / `lastCompactionError` from `next.last_context_usage` / `next.model_limits` / `next.compaction`.
  - In `resetProjection`, clear all four.
  - Export the new fields from the store's `return { ... }` block.

### GUI — `ContextMeter.vue` + popover + integration

- **Create** `apps/agent-gui/src/components/ContextMeter.vue`
  - Compact 6px segmented bar with one segment per `ContextSource` present in `lastContextUsage.by_source`. Empty state (no usage yet) shows a quiet placeholder bar.
  - Status badge: `>=70%` warn, `>=85%` err, `compacting` shows pulsing dot, `lastCompactionError` shows a warn icon with tooltip.
  - Click the bar → toggles a popover (absolute-positioned `<div>`, NOT `<dialog>`) with:
    - Per-source breakdown table (source label, est tokens, % of budget).
    - "Reserved for response" row.
    - Actions row: **Switch model…** button (disabled, `data-test="context-meter-switch-model"`) + **Compact now** button (`data-test="context-meter-compact"`).
  - Compact button → `invoke("compact_session")`, surfaces errors via `useToast`. Disabled while `session.compacting === true`.
- **Modify** `apps/agent-gui/src/styles/theme.css`
  - Add CSS custom properties for the per-source colours: `--src-system`, `--src-tools`, `--src-memory`, `--src-history`, `--src-tool-result`, `--src-selected-file`, `--src-compaction-summary`, `--src-request` (light + dark variants).
- **Modify** `apps/agent-gui/src/components/ChatPanel.vue`
  - Inject `<ContextMeter />` at the top of `<section class="chat-panel">`, above the existing `<header>`. The component is auto-registered by `unplugin-vue-components`.

### GUI — i18n

- **Modify** `apps/agent-gui/src/locales/en.json` and `apps/agent-gui/src/locales/zh-CN.json` (in lock-step)
  - New `context.*` group: `title`, `estimated`, `compactNow`, `compactInProgress`, `compactionFailed`, `switchModel`, `reservedForResponse`, `failedFallback`, `noUsageYet`, `popoverHeader`, `sourceSystem`, `sourceTools`, `sourceMemory`, `sourceHistory`, `sourceToolResult`, `sourceSelectedFile`, `sourceCompactionSummary`, `sourceRequest`, `percentOfBudget`.
  - New `status.compacting` and `status.contextNearFull` (alongside the existing `status.*` block).
  - New `errors.sessionBusy` (top-level new group).

### GUI — E2E mock

- **Modify** `apps/agent-gui/e2e/tauri-mock.js`
  - Add `compact_session` handler: emits `ContextCompactionStarted` then (after 50 ms) `CompactionSummary` and `ContextCompactionCompleted` so the UI can demo the busy → idle transition without a real LLM call.
  - Add `list_profiles_with_limits` handler: returns the mock profile list with synthetic `ModelLimits` per profile.
  - Update `start_session` / `switch_session` to seed a `ContextAssembled` event so the meter renders immediately.

### GUI — Vitest

- **Create** `apps/agent-gui/src/components/__tests__/ContextMeter.test.ts`
  - Three states: healthy (50% usage, no warn class), near-full (87% usage, err badge), compacting (pulsing dot, Compact button disabled).
  - Popover contents render every source row from `by_source` and the reserved-for-response row.
  - Compact button click invokes `compact_session`.
- **Modify** `apps/agent-gui/src/stores/__tests__/session.test.ts` — if it exists; else **create** it
  - Verify each new event variant updates store state correctly.

### TUI — `status_bar.rs` rendering extension

- **Modify** `crates/agent-tui/src/components/mod.rs`
  - Extend the existing `pub struct StatusInfo` with `context_usage: Option<agent_core::context_types::ContextUsage>` and `compacting: bool`. Update every literal `StatusInfo { … }` constructor in the crate to initialise both fields.
- **Modify** `crates/agent-tui/src/components/status_bar.rs`
  - Add `pub fn render_context_line_string(info: &StatusInfo, width: u16) -> String` returning the formatted text per spec §4.6 (long form when `width >= 100`; short form `ctx: 152.0k/200.0k (76%) ⚠` otherwise).
  - Switch `render_status_bar` to use the new context-line renderer when `info.context_usage.is_some()`; keep the existing legacy renderer body as the fallback for the no-usage case (preserves layout backward compatibility).
- **Modify** `crates/agent-tui/src/app.rs`
  - Add two private fields to `App`: `last_context_usage: Option<agent_core::context_types::ContextUsage>` and `compacting: bool` (both initialised to `None` / `false`).
  - In `handle_domain_event`, capture `ContextAssembled` / `ContextCompactionStarted` / `ContextCompactionCompleted` / `ContextCompactionFailed` and update those two fields. Mark dirty.
  - Update `sync_status_bar` to also populate the two new `StatusInfo` fields from `App::last_context_usage` / `App::compacting`.

### TUI — `:compact` command parsing + dispatch

- **Modify** `crates/agent-tui/src/components/mod.rs`
  - Add `Command::CompactSession { workspace_id: agent_core::WorkspaceId, session_id: agent_core::SessionId }` to `pub enum Command` (line 108).
- **Modify** `crates/agent-tui/src/components/chat.rs`
  - In `apply_key_action`, modify the `KeyAction::SendInput if !self.input_content.is_empty()` arm to first check `self.input_content.trim() == ":compact"`. If matched, clear the buffer and emit `Command::CompactSession { workspace_id, session_id }` instead of `SendMessage`. Otherwise fall through to the existing `SendMessage` path.
- **Modify** `crates/agent-tui/src/main.rs`
  - In `dispatch_commands`, add a `Command::CompactSession { workspace_id: _, session_id } => { … }` arm that awaits `runtime.compact_session(session_id, agent_core::CompactionReason::UserRequested)` and pushes an `[compact error: …]` ProjectedMessage on failure (mirrors the existing `Command::CancelSession` arm).

### TUI — integration tests

- **Modify** `crates/agent-tui/tests/app_logic.rs`
  - Add `colon_compact_input_dispatches_compact_session_command` — instantiates a `ChatPanel`, types `:compact`, sends `KeyAction::SendInput`, and asserts the resulting commands contain `Command::CompactSession` and NOT `Command::SendMessage`.

### `kairox.toml.example`

- No changes (P2 already added the `[context]` block).

---

## Task list (TDD, bite-sized)

The plan splits into 11 sequential tasks (Task 11 was originally a no-op verification task — REMOVED). Each task ends with running tests + a commit. Tasks 1–2 add the core projection fields. Tasks 3–4 add the Tauri commands + specta. Task 5 adds Pinia store reactive fields. Task 6 adds theme tokens + i18n. Task 7 builds the `ContextMeter.vue` component (with Vitest coverage). Task 8 wires `<ContextMeter />` into `ChatPanel.vue` + updates the E2E mock + adds Playwright spec. Task 9 ships the TUI status-bar context line. Task 10 ships the TUI `:compact` command. Task 12 final verification + push.

> **Reading order matters.** Type signatures defined in earlier tasks are referenced by name in later ones.

---

### Task 1 — Add `CompactionStatus` enum to `agent-core::projection`

**Files:**

- Modify: `crates/agent-core/src/projection.rs`
- Modify: `crates/agent-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Append to the existing `#[cfg(test)] mod tests` in `crates/agent-core/src/projection.rs`:

```rust
#[test]
fn compaction_status_serializes_with_internal_tag() {
    let s = CompactionStatus::Idle;
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "Idle");

    let s = CompactionStatus::Running;
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "Running");

    let s = CompactionStatus::Failed { error: "llm timeout".into() };
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["type"], "Failed");
    assert_eq!(json["error"], "llm timeout");

    let back: CompactionStatus = serde_json::from_value(json).unwrap();
    assert!(matches!(back, CompactionStatus::Failed { .. }));
}

#[test]
fn compaction_status_default_is_idle() {
    let s = CompactionStatus::default();
    assert!(matches!(s, CompactionStatus::Idle));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-core compaction_status`
Expected: FAIL — `cannot find type 'CompactionStatus' in this scope`.

- [ ] **Step 3: Add the enum**

Insert in `crates/agent-core/src/projection.rs` directly above `pub struct SessionProjection`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type")]
pub enum CompactionStatus {
    Idle,
    Running,
    Failed { error: String },
}

impl Default for CompactionStatus {
    fn default() -> Self { Self::Idle }
}

/// Mirror of `agent_models::ModelLimits` so projections survive the
/// `agent-core` ← `agent-models` dependency boundary. The runtime converts
/// on the boundary; field shape is kept in lock-step manually (Task 12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ProjectedModelLimits {
    pub context_window: u64,
    pub output_limit: u64,
    /// Snake-case `LimitSource` discriminant: "user_config" | "builtin_registry" | "runtime_probe" | "fallback".
    pub source: String,
}
```

- [ ] **Step 4: Re-export**

In `crates/agent-core/src/lib.rs`, locate the existing `pub use projection::{...}` line (or add one near other re-exports) and ensure both new types are exported:

```rust
pub use projection::{CompactionStatus, ProjectedMessage, ProjectedModelLimits, ProjectedRole, SessionProjection};
```

(Verify with `grep -n 'pub use projection' crates/agent-core/src/lib.rs` first; merge — never duplicate.)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p agent-core compaction_status`
Expected: PASS for both tests.

Run: `cargo test -p agent-core`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-core/src/projection.rs crates/agent-core/src/lib.rs
git commit -m "feat(core): add CompactionStatus + ProjectedModelLimits projection types"
```

---

### Task 2 — Project context-usage + compaction fields onto `SessionProjection`

**Files:**

- Modify: `crates/agent-core/src/projection.rs`

- [ ] **Step 1: Write the failing tests**

Append to the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn projects_context_assembled_into_last_context_usage() {
    use crate::context_types::{ContextSource, ContextUsage};
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let usage = ContextUsage {
        total_tokens: 12_000,
        budget_tokens: 180_000,
        context_window: 200_000,
        output_reservation: 20_000,
        by_source: vec![
            (ContextSource::System, 2_000),
            (ContextSource::History, 10_000),
        ],
        estimator: "cl100k_base",
        corrected_by_real_usage: false,
    };

    let event = DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextAssembled { usage: usage.clone() },
    );

    let projection = SessionProjection::from_events(&[event]);

    let cached = projection.last_context_usage.expect("usage should be set");
    assert_eq!(cached.total_tokens, 12_000);
    assert_eq!(cached.budget_tokens, 180_000);
    assert_eq!(cached.by_source.len(), 2);
}

#[test]
fn projects_compaction_lifecycle_into_compaction_status() {
    use crate::events::CompactionReason;
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();

    let started = DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextCompactionStarted {
            reason: CompactionReason::UserRequested,
            before_tokens: 180_000,
            candidate_event_count: 42,
        },
    );
    let completed = DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextCompactionCompleted {
            summary_id: "sum_1".into(),
            after_tokens: 30_000,
            fallback_used: false,
        },
    );

    let only_started = SessionProjection::from_events(&[started.clone()]);
    assert!(matches!(only_started.compaction, CompactionStatus::Running));

    let started_then_done = SessionProjection::from_events(&[started, completed]);
    assert!(matches!(started_then_done.compaction, CompactionStatus::Idle));
}

#[test]
fn projects_compaction_failed_into_failed_status() {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let failed = DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::ContextCompactionFailed {
            error: "model timeout".into(),
            fallback_used: true,
        },
    );

    let projection = SessionProjection::from_events(&[failed]);
    match projection.compaction {
        CompactionStatus::Failed { error } => assert_eq!(error, "model timeout"),
        other => panic!("expected Failed, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p agent-core projects_context_assembled projects_compaction`
Expected: FAIL — `no field 'last_context_usage' on type 'SessionProjection'`.

- [ ] **Step 3: Add fields to `SessionProjection`**

In `crates/agent-core/src/projection.rs`, modify the struct (keep existing fields, add three with `#[serde(default)]` so back-compat persists):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SessionProjection {
    pub messages: Vec<ProjectedMessage>,
    pub task_titles: Vec<String>,
    pub task_graph: TaskGraphSnapshot,
    pub token_stream: String,
    pub cancelled: bool,
    #[serde(default)]
    pub last_context_usage: Option<crate::context_types::ContextUsage>,
    #[serde(default)]
    pub model_limits: Option<ProjectedModelLimits>,
    #[serde(default)]
    pub compaction: CompactionStatus,
}
```

- [ ] **Step 4: Add the `apply` arms**

In `SessionProjection::apply`, locate the long catch-all `match` arm at the bottom (the chain ending in `EventPayload::CompactionSummary { .. } => {}`). Move these four variants OUT of the catch-all and add explicit handling **before** the catch-all:

```rust
EventPayload::ContextAssembled { usage } => {
    self.last_context_usage = Some(usage.clone());
}
EventPayload::ContextCompactionStarted { .. } => {
    self.compaction = CompactionStatus::Running;
}
EventPayload::ContextCompactionCompleted { .. } => {
    self.compaction = CompactionStatus::Idle;
}
EventPayload::ContextCompactionFailed { error, .. } => {
    self.compaction = CompactionStatus::Failed { error: error.clone() };
}
```

Then **delete** those four variants from the long `EventPayload::WorkspaceOpened { .. } | ... | EventPayload::CompactionSummary { .. } => {}` chain so the match stays exhaustive without overlap. `CompactionSummary` stays in the catch-all.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p agent-core projects_context_assembled projects_compaction`
Expected: PASS for all three new tests.

Run: `cargo test -p agent-core`
Expected: all green (existing `serializes_projection_with_snake_case_roles` continues to pass — new fields default to `None`/`Idle` when omitted).

- [ ] **Step 6: Commit**

```bash
git add crates/agent-core/src/projection.rs
git commit -m "feat(core): project context usage + compaction lifecycle into SessionProjection"
```

---

### Task 3 — Add `compact_session` Tauri command

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`

- [ ] **Step 1: Verify the runtime API is in place**

Run: `grep -n 'pub async fn compact_session' crates/agent-runtime/src/facade_runtime.rs`
Expected: a single match showing the signature `pub async fn compact_session(&self, session_id: SessionId, reason: CompactionReason) -> Result<()>` (added in P2).

If missing — STOP and surface a blocker. P2 must already ship this.

Also verify: `grep -n 'pub use events::CompactionReason\|pub use.*CompactionReason' crates/agent-core/src/lib.rs`
If missing, add `pub use events::CompactionReason;` to `crates/agent-core/src/lib.rs` as a tiny first commit:

```bash
git add crates/agent-core/src/lib.rs
git commit -m "chore(core): re-export CompactionReason from agent-core root"
```

- [ ] **Step 2: Write the failing test**

Append to `apps/agent-gui/src-tauri/src/commands.rs`:

```rust
#[cfg(test)]
mod compact_session_command_tests {
    use super::compact_session;

    #[test]
    fn compact_session_command_function_exists() {
        // Compile-time presence check — if `compact_session` is renamed or
        // removed this fails to compile, which is exactly the signal we want
        // before `collect_commands![]` / `generate_handler![]` blow up.
        let _ = compact_session;
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p agent-gui-tauri compact_session_command_function_exists`
Expected: FAIL — `cannot find function 'compact_session' in this scope`.

- [ ] **Step 4: Implement the command**

Append to `apps/agent-gui/src-tauri/src/commands.rs` (next to `cancel_session`):

```rust
#[tauri::command]
#[specta::specta]
pub async fn compact_session(state: State<'_, GuiState>) -> Result<(), String> {
    let session_id = {
        let current = state.current_session_id.lock().await;
        current
            .clone()
            .ok_or_else(|| "No active session to compact".to_string())?
    };

    state
        .runtime
        .compact_session(session_id, agent_core::CompactionReason::UserRequested)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 5: Register in `generate_handler!` and `collect_commands!`**

In `apps/agent-gui/src-tauri/src/lib.rs` `generate_handler![...]`, add `crate::commands::compact_session,` after `crate::commands::cancel_session,`.

In `apps/agent-gui/src-tauri/src/specta.rs` `collect_commands![...]`, add `compact_session,` after `cancel_session,`.

- [ ] **Step 6: Run tests to verify it passes**

Run: `cargo test -p agent-gui-tauri compact_session_command_function_exists`
Expected: PASS.

Run: `cargo build -p agent-gui-tauri`
Expected: clean build (the `collect_commands![]` / `generate_handler![]` macros expand correctly).

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src-tauri/src/lib.rs apps/agent-gui/src-tauri/src/specta.rs
git commit -m "feat(gui): add compact_session Tauri command"
```

---

### Task 4 — Add `list_profiles_with_limits` Tauri command + `ProfileWithLimits` DTO

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`

- [ ] **Step 1: Write the failing test**

Append to `apps/agent-gui/src-tauri/src/commands.rs`:

```rust
#[cfg(test)]
mod profile_with_limits_tests {
    use super::*;

    #[test]
    fn profile_with_limits_serializes_expected_shape() {
        let p = ProfileWithLimits {
            alias: "fast".into(),
            provider: "openai".into(),
            model_id: "gpt-4o-mini".into(),
            context_window: 128_000,
            output_limit: 16_384,
            limit_source: "builtin_registry".into(),
            has_api_key: true,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"alias\":\"fast\""));
        assert!(json.contains("\"context_window\":128000"));
        assert!(json.contains("\"limit_source\":\"builtin_registry\""));
        assert!(json.contains("\"has_api_key\":true"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-gui-tauri profile_with_limits_serializes_expected_shape`
Expected: FAIL — `cannot find type 'ProfileWithLimits'`.

- [ ] **Step 3: Add the DTO**

Append to `apps/agent-gui/src-tauri/src/commands.rs` (near other DTOs):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProfileWithLimits {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub context_window: u64,
    pub output_limit: u64,
    /// Snake-case `LimitSource`: "user_config" | "builtin_registry" | "runtime_probe" | "fallback".
    pub limit_source: String,
    pub has_api_key: bool,
}
```

- [ ] **Step 4: Add the command**

> **Verified facts (do not change without re-checking with grep):**
>
> - `agent_config::Config::profiles` is `pub profiles: Vec<(String, ProfileDef)>` (NOT a `BTreeMap`). Iterate by `&(alias, profile)` pairs.
> - `ProfileDef` has BOTH `api_key: Option<String>` (direct value, takes priority) AND `api_key_env: Option<String>`. `has_api_key` must be true if either provides a key. Local providers (e.g. `provider == "ollama"` or `provider == "fake"`) need no key — treat them as `has_api_key: true`.
> - `agent_config::resolve_limits(profile: &ProfileDef) -> agent_models::ModelLimits` is the canonical resolver and is already re-exported from `agent_config`.
> - `agent_models::LimitSource` variants are exactly: `UserConfig | BuiltinRegistry | RuntimeProbe | Fallback`.

Append to `apps/agent-gui/src-tauri/src/commands.rs`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn list_profiles_with_limits(
    state: State<'_, GuiState>,
) -> Result<Vec<ProfileWithLimits>, String> {
    let mut out = Vec::with_capacity(state.config.profiles.len());
    for (alias, profile) in &state.config.profiles {
        let limits = agent_config::resolve_limits(profile);
        let limit_source = match limits.source {
            agent_models::LimitSource::UserConfig => "user_config",
            agent_models::LimitSource::BuiltinRegistry => "builtin_registry",
            agent_models::LimitSource::RuntimeProbe => "runtime_probe",
            agent_models::LimitSource::Fallback => "fallback",
        };
        let has_api_key = profile.api_key.is_some()
            || profile
                .api_key_env
                .as_deref()
                .map(|env| std::env::var(env).is_ok())
                .unwrap_or(false)
            || matches!(profile.provider.as_str(), "ollama" | "fake");
        out.push(ProfileWithLimits {
            alias: alias.clone(),
            provider: profile.provider.clone(),
            model_id: profile.model_id.clone(),
            context_window: limits.context_window,
            output_limit: limits.output_limit,
            limit_source: limit_source.into(),
            has_api_key,
        });
    }
    Ok(out)
}
```

> **Cross-check** before moving on: `grep -n 'pub api_key:\|pub api_key_env:\|pub provider:\|pub model_id:' crates/agent-config/src/lib.rs` should show the four field names used above. If any differs (e.g. `provider` is renamed), update accordingly.

- [ ] **Step 5: Register the command + DTO + `CompactionStatus`/`ProjectedModelLimits`**

In `apps/agent-gui/src-tauri/src/lib.rs` `generate_handler![...]`, add `crate::commands::list_profiles_with_limits,` after `crate::commands::list_profiles,`.

In `apps/agent-gui/src-tauri/src/specta.rs`:

- Add `list_profiles_with_limits,` to `collect_commands![...]` after `list_profiles,`.
- Add the new types to the imports at top: change the existing `use agent_core::{...};` to also bring in `CompactionStatus` and `ProjectedModelLimits`.
- Add `.typ::<ProfileWithLimits>()` after `.typ::<ProfileDetailResponse>()`.
- Add `.typ::<CompactionStatus>()` and `.typ::<ProjectedModelLimits>()` near the other context-mgmt registrations (`.typ::<ContextUsage>()` etc.).

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p agent-gui-tauri profile_with_limits_serializes_expected_shape`
Expected: PASS.

Run: `cargo build -p agent-gui-tauri`
Expected: clean build.

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs apps/agent-gui/src-tauri/src/lib.rs apps/agent-gui/src-tauri/src/specta.rs
git commit -m "feat(gui): add list_profiles_with_limits Tauri command + ProfileWithLimits DTO"
```

---

### Task 5 — Pinia `useSessionStore` reactive context fields + event handlers

**Files:**

- Modify: `apps/agent-gui/src/stores/session.ts`

> **Background:** P3 must regenerate `apps/agent-gui/src/generated/{commands,events}.ts` before this task can typecheck — the new fields (`last_context_usage`, `model_limits`, `compaction`) and the `ProfileWithLimits` / `CompactionStatus` types must be available in the TypeScript surface.
>
> Run `just gen-types` once at the start of this task. Confirm with `git diff --stat apps/agent-gui/src/generated/` that both files updated. Commit immediately:
>
> ```bash
> git add apps/agent-gui/src/generated/
> git commit -m "chore(gui): regenerate specta bindings for P3 commands + projection fields"
> ```

- [ ] **Step 1: Write the failing test**

Create `apps/agent-gui/src/stores/__tests__/session-context.test.ts` (new file — `__tests__` may not exist yet; create the directory):

```typescript
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, beforeEach } from "vitest";
import { useSessionStore } from "@/stores/session";
import type { DomainEvent, ContextUsage } from "@/types";

function makeUsage(overrides: Partial<ContextUsage> = {}): ContextUsage {
  return {
    total_tokens: 12_000,
    budget_tokens: 180_000,
    context_window: 200_000,
    output_reservation: 20_000,
    by_source: [
      ["System", 2_000],
      ["History", 10_000]
    ],
    estimator: "cl100k_base",
    corrected_by_real_usage: false,
    ...overrides
  } as ContextUsage;
}

function makeEvent(payload: DomainEvent["payload"]): DomainEvent {
  return {
    schema_version: 1,
    workspace_id: "wrk_test",
    session_id: "ses_test",
    timestamp: new Date().toISOString(),
    source_agent_id: "agent_system",
    privacy: "minimal_trace" as const,
    event_type: payload.type,
    payload
  } as DomainEvent;
}

describe("useSessionStore — context fields", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("starts with null context usage and idle compaction", () => {
    const session = useSessionStore();
    expect(session.lastContextUsage).toBeNull();
    expect(session.modelLimits).toBeNull();
    expect(session.compacting).toBe(false);
    expect(session.lastCompactionError).toBeNull();
  });

  it("captures ContextAssembled into lastContextUsage", () => {
    const session = useSessionStore();
    session.applyEvent(makeEvent({ type: "ContextAssembled", usage: makeUsage() }));
    expect(session.lastContextUsage?.total_tokens).toBe(12_000);
    expect(session.lastContextUsage?.budget_tokens).toBe(180_000);
  });

  it("flips compacting on ContextCompactionStarted/Completed/Failed", () => {
    const session = useSessionStore();

    session.applyEvent(
      makeEvent({
        type: "ContextCompactionStarted",
        reason: { type: "UserRequested" },
        before_tokens: 180_000,
        candidate_event_count: 12
      })
    );
    expect(session.compacting).toBe(true);

    session.applyEvent(
      makeEvent({
        type: "ContextCompactionCompleted",
        summary_id: "sum_1",
        after_tokens: 30_000,
        fallback_used: false
      })
    );
    expect(session.compacting).toBe(false);
    expect(session.lastCompactionError).toBeNull();

    session.applyEvent(
      makeEvent({
        type: "ContextCompactionFailed",
        error: "model timeout",
        fallback_used: true
      })
    );
    expect(session.compacting).toBe(false);
    expect(session.lastCompactionError).toBe("model timeout");
  });

  it("hydrates from setProjection and resets via resetProjection", () => {
    const session = useSessionStore();
    session.setProjection({
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false,
      last_context_usage: makeUsage({ total_tokens: 50_000 }),
      model_limits: { context_window: 200_000, output_limit: 8_192, source: "builtin_registry" },
      compaction: { type: "Running" }
    } as never);
    expect(session.lastContextUsage?.total_tokens).toBe(50_000);
    expect(session.modelLimits?.context_window).toBe(200_000);
    expect(session.compacting).toBe(true);

    session.resetProjection();
    expect(session.lastContextUsage).toBeNull();
    expect(session.modelLimits).toBeNull();
    expect(session.compacting).toBe(false);
    expect(session.lastCompactionError).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/agent-gui && pnpm exec vitest run src/stores/__tests__/session-context.test.ts`
Expected: FAIL — `lastContextUsage` / `modelLimits` / `compacting` / `lastCompactionError` undefined on the store.

- [ ] **Step 3: Re-export new types from `@/types`**

> **Verified facts:**
>
> - `apps/agent-gui/src/types/index.ts` is a hand-written re-export hub. `SessionProjection` is defined here as a hand-written interface (NOT generated by specta), with fields `messages`, `task_titles`, `task_graph`, `token_stream`, `cancelled`. P3 must add the three new fields here as well.
> - `apps/agent-gui/src/stores/session.ts` line 16 has a private `function emptyProjection()` that returns the hand-written shape — it must be extended with the three new fields too.
> - `ContextUsage`, `ProjectedModelLimits`, `CompactionStatus` are NOT yet re-exported from `@/types/index.ts`. Add the re-export.

In `apps/agent-gui/src/types/index.ts`, add to the `// ===== Auto-generated types (from specta) =====` block:

```typescript
export type {
  EventPayload,
  DomainEvent,
  AgentRole,
  TaskState,
  TaskSnapshot,
  TaskGraphSnapshot,
  PrivacyClassification,
  MemoryScope,
  ContextSource,
  ContextUsage,
  ProjectedModelLimits,
  CompactionStatus,
  ModelLimits,
  LimitSource,
  CompactionReason
} from "../generated/events";
```

Then update the hand-written `SessionProjection` interface (replace the existing block):

```typescript
export interface SessionProjection {
  messages: ProjectedMessage[];
  task_titles: string[];
  task_graph: TaskGraphSnapshot;
  token_stream: string;
  cancelled: boolean;
  /** P3: last context usage snapshot (set by ContextAssembled events). */
  last_context_usage: ContextUsage | null;
  /** P3: resolved model limits for the current profile. */
  model_limits: ProjectedModelLimits | null;
  /** P3: compaction lifecycle status (Idle / Running / Failed). */
  compaction: CompactionStatus;
}
```

> The two new types `ContextUsage` and `ProjectedModelLimits` and `CompactionStatus` must already be referenced — add an explicit `import type { ContextUsage, ProjectedModelLimits, CompactionStatus } from "../generated/events";` at the top of `index.ts` if needed (the `export type` re-export above doesn't make them visible to other declarations in the same file).

- [ ] **Step 4: Update `emptyProjection()` + add reactive state**

In `apps/agent-gui/src/stores/session.ts`, modify the existing import + add new ones:

```typescript
import type {
  SessionProjection,
  SessionInfoResponse,
  DomainEvent,
  ContextUsage,
  ProjectedModelLimits
} from "@/types";
```

Replace `function emptyProjection()` (currently lines ~16-23):

```typescript
function emptyProjection(): SessionProjection {
  return {
    messages: [],
    task_titles: [],
    task_graph: { tasks: [] },
    token_stream: "",
    cancelled: false,
    last_context_usage: null,
    model_limits: null,
    compaction: { type: "Idle" }
  };
}
```

Inside the `defineStore` setup function, immediately after the existing `const currentProfile = ref<string>("fast");` line, insert:

```typescript
const lastContextUsage = ref<ContextUsage | null>(null);
const modelLimits = ref<ProjectedModelLimits | null>(null);
const compacting = ref(false);
const lastCompactionError = ref<string | null>(null);
```

- [ ] **Step 5: Add explicit case arms + remove `ContextAssembled` from no-op chain**

> **Verified fact:** the no-op chain in `apps/agent-gui/src/stores/session.ts` lives at lines 125-145, listing `case "AgentSpawned":`, `case "AgentIdle":`, …, `case "ContextAssembled":` (line 129), …, all falling through to a single `break;`. We must remove the `case "ContextAssembled":` line and add four NEW explicit case blocks before the chain.

In `applyEvent`'s `switch (p.type)`, find the case block ending with `case "TaskRetried": { … break; }` (around line 118-124). Immediately AFTER that block and BEFORE `case "AgentSpawned":` (the start of the no-op chain), insert:

```typescript
case "ContextAssembled": {
  lastContextUsage.value = p.usage;
  break;
}
case "ContextCompactionStarted": {
  compacting.value = true;
  lastCompactionError.value = null;
  break;
}
case "ContextCompactionCompleted": {
  compacting.value = false;
  break;
}
case "ContextCompactionFailed": {
  compacting.value = false;
  lastCompactionError.value = p.error;
  break;
}
```

Then in the no-op chain (the `case "AgentSpawned": case "AgentIdle": … case "ContextAssembled": …` fall-through block), DELETE just the `case "ContextAssembled":` line. The other fall-through cases stay untouched.

- [ ] **Step 6: Hydrate the four context refs in `setProjection` and clear them in `resetProjection`**

> **Verified facts** (from `grep -n 'function setProjection\|function resetProjection\|emptyProjection\|projection.value' apps/agent-gui/src/stores/session.ts | cat` against the current snapshot):
>
> - `useSessionStore` exports `setProjection(next: SessionProjection)` (defined at `apps/agent-gui/src/stores/session.ts:149-155`) — this is the canonical hydration entry point used by `switchSession` (line 178) after the Tauri `switch_session` round-trip, and by any future restore path.
> - `useSessionStore` exports `resetProjection()` (defined at `apps/agent-gui/src/stores/session.ts:157-162`) — called by `switchSession`, `createSession`, and `deleteSession` to clear local state before the next projection arrives.
> - `applyEvent` (Step 5) handles the live event-driven path; `setProjection` / `resetProjection` handle the snapshot-driven path. Both paths must keep the four new refs in sync, otherwise a session restored from server state will show a stale meter.

In `apps/agent-gui/src/stores/session.ts`, modify `setProjection` (currently lines 149-155):

```typescript
function setProjection(next: SessionProjection) {
  projection.value = next;
  isStreaming.value = false;
  if (next.task_graph?.tasks) {
    useTaskGraphStore().setTaskGraph(next.task_graph.tasks, currentSessionId.value);
  }
  // P3: hydrate context refs from the projection snapshot.
  lastContextUsage.value = next.last_context_usage ?? null;
  modelLimits.value = next.model_limits ?? null;
  compacting.value = next.compaction.type === "Running";
  lastCompactionError.value = next.compaction.type === "Failed" ? next.compaction.error : null;
}
```

And `resetProjection` (currently lines 157-162) — append four lines before the closing `}`:

```typescript
function resetProjection() {
  projection.value = emptyProjection();
  isStreaming.value = false;
  streamsByTask.value.clear();
  useAgentsStore().clearAgents();
  // P3: clear context refs.
  lastContextUsage.value = null;
  modelLimits.value = null;
  compacting.value = false;
  lastCompactionError.value = null;
}
```

> **Type note:** Task 2 added `last_context_usage`, `model_limits` (both `Option<…>`) and `compaction: CompactionStatus` to `SessionProjection`. After `just gen-types` runs in Task 12, these surface in TypeScript as `last_context_usage: ContextUsage | null`, `model_limits: ProjectedModelLimits | null`, `compaction: CompactionStatus`. The hand-rolled `types/index.ts` re-export added in Task 5 Step 1 mirrors the same shape so this code compiles before Task 12 too.

- [ ] **Step 7: Add a test that `setProjection` hydrates and `resetProjection` clears**

Append to `apps/agent-gui/src/stores/__tests__/session-context.test.ts`:

```typescript
it("setProjection hydrates context refs from the snapshot", () => {
  const session = useSessionStore();
  session.setProjection({
    messages: [],
    task_graph: { tasks: [], root_id: null },
    last_context_usage: makeUsage(),
    model_limits: { context_window: 200_000, output_limit: 20_000, source: "user_config" },
    compaction: { type: "Running" }
  } as unknown as SessionProjection);

  expect(session.lastContextUsage).not.toBeNull();
  expect(session.modelLimits?.context_window).toBe(200_000);
  expect(session.compacting).toBe(true);
  expect(session.lastCompactionError).toBeNull();
});

it("setProjection surfaces the failed-compaction error", () => {
  const session = useSessionStore();
  session.setProjection({
    messages: [],
    task_graph: { tasks: [], root_id: null },
    last_context_usage: null,
    model_limits: null,
    compaction: { type: "Failed", error: "model timeout" }
  } as unknown as SessionProjection);

  expect(session.compacting).toBe(false);
  expect(session.lastCompactionError).toBe("model timeout");
});

it("resetProjection clears the four context refs", () => {
  const session = useSessionStore();
  // Seed non-default values via setProjection first.
  session.setProjection({
    messages: [],
    task_graph: { tasks: [], root_id: null },
    last_context_usage: makeUsage(),
    model_limits: { context_window: 200_000, output_limit: 20_000, source: "user_config" },
    compaction: { type: "Running" }
  } as unknown as SessionProjection);

  session.resetProjection();

  expect(session.lastContextUsage).toBeNull();
  expect(session.modelLimits).toBeNull();
  expect(session.compacting).toBe(false);
  expect(session.lastCompactionError).toBeNull();
});
```

> Add `makeUsage()` helper at the top of the test file if not already present:
>
> ```typescript
> function makeUsage(): ContextUsage {
>   return {
>     total_tokens: 90_000,
>     budget_tokens: 180_000,
>     context_window: 200_000,
>     output_reservation: 20_000,
>     by_source: [
>       ["System", 2_000],
>       ["History", 88_000]
>     ],
>     estimator: "cl100k_base",
>     corrected_by_real_usage: false
>   };
> }
> ```

- [ ] **Step 8: Export new fields**

In the store's `return { ... }` block, add (in the state section, alongside `currentProfile`):

```typescript
lastContextUsage,
modelLimits,
compacting,
lastCompactionError,
```

- [ ] **Step 9: Run tests to verify they pass**

Run: `cd apps/agent-gui && pnpm exec vitest run src/stores/__tests__/session-context.test.ts`
Expected: PASS for all tests.

Run: `cd apps/agent-gui && pnpm exec vitest run`
Expected: all green (no other store tests broken).

- [ ] **Step 10: Commit**

```bash
git add apps/agent-gui/src/stores/session.ts apps/agent-gui/src/stores/__tests__/session-context.test.ts apps/agent-gui/src/types/index.ts
git commit -m "feat(gui): add context-usage + compaction reactive fields to useSessionStore"
```

---

### Task 6 — Theme tokens for per-source colours + i18n keys

**Files:**

- Modify: `apps/agent-gui/src/styles/theme.css`
- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`

- [ ] **Step 1: Add CSS custom properties (light defaults)**

In `apps/agent-gui/src/styles/theme.css`, locate the `:root { ... }` block and append the per-source colour tokens (use existing palette colours where possible):

```css
:root {
  /* … existing tokens … */

  /* Context meter — per-source colours (light) */
  --src-system: #6b7280; /* slate-500 */
  --src-tools: #2563eb; /* blue-600 */
  --src-memory: #7c3aed; /* violet-600 */
  --src-history: #16a34a; /* green-600 */
  --src-tool-result: #f59e0b; /* amber-500 */
  --src-selected-file: #db2777; /* pink-600 */
  --src-compaction-summary: #9ca3af; /* slate-400 */
  --src-request: #0ea5e9; /* sky-500 */
}
```

In the `html.dark { ... }` block, add darker / higher-contrast variants:

```css
html.dark {
  /* … existing dark overrides … */

  --src-system: #94a3b8;
  --src-tools: #60a5fa;
  --src-memory: #a78bfa;
  --src-history: #4ade80;
  --src-tool-result: #fbbf24;
  --src-selected-file: #f472b6;
  --src-compaction-summary: #6b7280;
  --src-request: #38bdf8;
}
```

- [ ] **Step 2: Add i18n keys to `en.json`**

Insert a new top-level `context` block in `apps/agent-gui/src/locales/en.json` (alphabetical order, between `chat` and `marketplace`):

```json
"context": {
  "title": "Context",
  "estimated": "estimated",
  "compactNow": "Compact now",
  "compactInProgress": "Compacting…",
  "compactionFailed": "Compaction failed: {error}",
  "switchModel": "Switch model…",
  "reservedForResponse": "Reserved for response",
  "failedFallback": "Used sliding-window fallback",
  "noUsageYet": "No usage yet",
  "popoverHeader": "Context window",
  "percentOfBudget": "{pct}% of budget",
  "sourceSystem": "System",
  "sourceTools": "Tool definitions",
  "sourceMemory": "Memory",
  "sourceHistory": "Conversation history",
  "sourceToolResult": "Tool results",
  "sourceSelectedFile": "Selected files",
  "sourceCompactionSummary": "Compaction summary",
  "sourceRequest": "User request"
}
```

In the existing `status` block, add two keys:

```json
"compacting": "Compacting context…",
"contextNearFull": "Context window nearly full"
```

Add a new top-level `errors` block (after `notifications`):

```json
"errors": {
  "sessionBusy": "Session is busy: {reason}"
}
```

- [ ] **Step 3: Add the same keys to `zh-CN.json`**

Mirror the structure with translated values:

```json
"context": {
  "title": "上下文",
  "estimated": "估算",
  "compactNow": "立即压缩",
  "compactInProgress": "压缩中…",
  "compactionFailed": "压缩失败：{error}",
  "switchModel": "切换模型…",
  "reservedForResponse": "为模型回复预留",
  "failedFallback": "已使用滑动窗口回退",
  "noUsageYet": "暂无用量数据",
  "popoverHeader": "上下文窗口",
  "percentOfBudget": "占预算 {pct}%",
  "sourceSystem": "系统",
  "sourceTools": "工具定义",
  "sourceMemory": "记忆",
  "sourceHistory": "对话历史",
  "sourceToolResult": "工具结果",
  "sourceSelectedFile": "选中的文件",
  "sourceCompactionSummary": "压缩摘要",
  "sourceRequest": "用户请求"
}
```

`status` block additions:

```json
"compacting": "正在压缩上下文…",
"contextNearFull": "上下文窗口接近满载"
```

`errors` block:

```json
"errors": {
  "sessionBusy": "会话繁忙：{reason}"
}
```

- [ ] **Step 4: Verify i18n parses**

Run: `cd apps/agent-gui && pnpm exec vue-tsc --noEmit`
Expected: clean (no JSON parse errors; vue-i18n loaders type-check the messages).

Run: `cd apps/agent-gui && node -e "JSON.parse(require('fs').readFileSync('src/locales/en.json'));JSON.parse(require('fs').readFileSync('src/locales/zh-CN.json'));console.log('OK')"`
Expected: prints `OK`.

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src/styles/theme.css apps/agent-gui/src/locales/en.json apps/agent-gui/src/locales/zh-CN.json
git commit -m "feat(gui): add context-meter theme tokens + i18n keys (en, zh-CN)"
```

---

### Task 7 — Build `ContextMeter.vue` component

**Files:**

- Create: `apps/agent-gui/src/components/ContextMeter.vue`

- [ ] **Step 1: Write the failing test**

Create `apps/agent-gui/src/components/__tests__/ContextMeter.test.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import ContextMeter from "@/components/ContextMeter.vue";
import { useSessionStore } from "@/stores/session";
import type { ContextUsage } from "@/types";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invokeMock(...args) }));

function usage(overrides: Partial<ContextUsage> = {}): ContextUsage {
  return {
    total_tokens: 90_000,
    budget_tokens: 180_000,
    context_window: 200_000,
    output_reservation: 20_000,
    by_source: [
      ["System", 2_000],
      ["ToolDefinitions", 22_000],
      ["History", 60_000],
      ["Memory", 6_000]
    ],
    estimator: "cl100k_base",
    corrected_by_real_usage: false,
    ...overrides
  } as ContextUsage;
}

describe("ContextMeter.vue", () => {
  beforeEach(() => invokeMock.mockReset());

  it("renders a placeholder when no usage is available yet", () => {
    const wrapper = mountWithPlugins(ContextMeter);
    expect(wrapper.find('[data-test="context-meter-empty"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="context-meter-bar"]').exists()).toBe(false);
  });

  it("renders the segmented bar and a healthy badge under 70%", () => {
    const wrapper = mountWithPlugins(ContextMeter);
    const session = useSessionStore();
    session.lastContextUsage = usage({ total_tokens: 90_000 }); // 50%
    return wrapper.vm.$nextTick().then(() => {
      expect(wrapper.find('[data-test="context-meter-bar"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="context-meter-badge-warn"]').exists()).toBe(false);
      expect(wrapper.find('[data-test="context-meter-badge-err"]').exists()).toBe(false);
    });
  });

  it("shows the err badge above 85%", async () => {
    const wrapper = mountWithPlugins(ContextMeter);
    const session = useSessionStore();
    session.lastContextUsage = usage({ total_tokens: 160_000 }); // ~88%
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="context-meter-badge-err"]').exists()).toBe(true);
  });

  it("disables the Compact button while compacting", async () => {
    const wrapper = mountWithPlugins(ContextMeter);
    const session = useSessionStore();
    session.lastContextUsage = usage();
    session.compacting = true;
    await wrapper.vm.$nextTick();
    // Open the popover first
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    const btn = wrapper.find<HTMLButtonElement>('[data-test="context-meter-compact"]');
    expect(btn.exists()).toBe(true);
    expect(btn.element.disabled).toBe(true);
  });

  it("invokes compact_session when Compact is clicked", async () => {
    invokeMock.mockResolvedValue(undefined);
    const wrapper = mountWithPlugins(ContextMeter);
    const session = useSessionStore();
    session.lastContextUsage = usage();
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    await wrapper.find('[data-test="context-meter-compact"]').trigger("click");
    expect(invokeMock).toHaveBeenCalledWith("compact_session");
  });

  it("renders one popover row per source from by_source", async () => {
    const wrapper = mountWithPlugins(ContextMeter);
    const session = useSessionStore();
    session.lastContextUsage = usage();
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="context-meter-bar"]').trigger("click");
    const rows = wrapper.findAll('[data-test^="context-meter-row-"]');
    expect(rows.length).toBe(4);
    expect(wrapper.find('[data-test="context-meter-reserved"]').exists()).toBe(true);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd apps/agent-gui && pnpm exec vitest run src/components/__tests__/ContextMeter.test.ts`
Expected: FAIL — module `@/components/ContextMeter.vue` not found.

- [ ] **Step 3: Implement the component**

Create `apps/agent-gui/src/components/ContextMeter.vue`:

```vue
<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";
import { useToast } from "@/composables/useToast";
import type { ContextSource } from "@/types";

const { t } = useI18n();
const session = useSessionStore();
const toast = useToast();
const popoverOpen = ref(false);

const ratio = computed(() => {
  const u = session.lastContextUsage;
  if (!u || u.budget_tokens === 0) return 0;
  return Math.min(1, u.total_tokens / u.budget_tokens);
});

const ratioPct = computed(() => Math.round(ratio.value * 100));

const badgeKind = computed<"healthy" | "warn" | "err">(() => {
  if (ratio.value >= 0.85) return "err";
  if (ratio.value >= 0.7) return "warn";
  return "healthy";
});

const sourceColorVar: Record<ContextSource, string> = {
  System: "var(--src-system)",
  ToolDefinitions: "var(--src-tools)",
  Memory: "var(--src-memory)",
  History: "var(--src-history)",
  ToolResult: "var(--src-tool-result)",
  SelectedFile: "var(--src-selected-file)",
  CompactionSummary: "var(--src-compaction-summary)",
  Request: "var(--src-request)"
};

const sourceLabel: Record<ContextSource, string> = {
  System: "context.sourceSystem",
  ToolDefinitions: "context.sourceTools",
  Memory: "context.sourceMemory",
  History: "context.sourceHistory",
  ToolResult: "context.sourceToolResult",
  SelectedFile: "context.sourceSelectedFile",
  CompactionSummary: "context.sourceCompactionSummary",
  Request: "context.sourceRequest"
};

function formatTokens(n: number): string {
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

function togglePopover() {
  if (!session.lastContextUsage) return;
  popoverOpen.value = !popoverOpen.value;
}

async function onCompactClick() {
  if (session.compacting) return;
  popoverOpen.value = false;
  try {
    await invoke("compact_session");
  } catch (e) {
    toast.error(t("context.compactionFailed", { error: String(e) }));
  }
}
</script>

<template>
  <div class="context-meter" data-test="context-meter">
    <div v-if="!session.lastContextUsage" class="empty" data-test="context-meter-empty">
      <span class="empty-bar" />
      <span class="empty-label">{{ t("context.noUsageYet") }}</span>
    </div>

    <div v-else class="meter-row">
      <button
        type="button"
        class="bar"
        data-test="context-meter-bar"
        :title="t('context.popoverHeader')"
        @click="togglePopover"
      >
        <span
          v-for="[source, tokens] in session.lastContextUsage.by_source"
          :key="source"
          class="segment"
          :style="{
            width: `${(tokens / session.lastContextUsage.budget_tokens) * 100}%`,
            background: sourceColorVar[source as ContextSource]
          }"
        />
      </button>

      <span class="numbers" data-test="context-meter-numbers">
        {{ formatTokens(session.lastContextUsage.total_tokens) }} /
        {{ formatTokens(session.lastContextUsage.budget_tokens) }}
        ({{ ratioPct }}%)
      </span>

      <span v-if="session.compacting" class="badge badge-busy" data-test="context-meter-badge-busy">
        <span class="dot" />
        {{ t("context.compactInProgress") }}
      </span>
      <span
        v-else-if="badgeKind === 'err'"
        class="badge badge-err"
        data-test="context-meter-badge-err"
      >
        ⚠ {{ t("status.contextNearFull") }}
      </span>
      <span
        v-else-if="badgeKind === 'warn'"
        class="badge badge-warn"
        data-test="context-meter-badge-warn"
      >
        ⚠
      </span>

      <span
        v-if="session.lastCompactionError"
        class="badge badge-warn"
        data-test="context-meter-badge-failed"
        :title="session.lastCompactionError"
      >
        ⚠ {{ t("context.failedFallback") }}
      </span>
    </div>

    <div
      v-if="popoverOpen && session.lastContextUsage"
      class="popover"
      data-test="context-meter-popover"
    >
      <header class="popover-header">{{ t("context.popoverHeader") }}</header>
      <table class="popover-table">
        <tbody>
          <tr
            v-for="[source, tokens] in session.lastContextUsage.by_source"
            :key="source"
            :data-test="`context-meter-row-${source}`"
          >
            <td>
              <span
                class="swatch"
                :style="{ background: sourceColorVar[source as ContextSource] }"
              />
              {{ t(sourceLabel[source as ContextSource]) }}
            </td>
            <td>{{ formatTokens(tokens) }}</td>
            <td>
              {{
                t("context.percentOfBudget", {
                  pct: Math.round((tokens / session.lastContextUsage.budget_tokens) * 100)
                })
              }}
            </td>
          </tr>
          <tr data-test="context-meter-reserved">
            <td>{{ t("context.reservedForResponse") }}</td>
            <td>{{ formatTokens(session.lastContextUsage.output_reservation) }}</td>
            <td></td>
          </tr>
        </tbody>
      </table>
      <div class="popover-actions">
        <button
          type="button"
          class="btn btn-ghost"
          data-test="context-meter-switch-model"
          disabled
          :title="t('context.switchModel')"
        >
          {{ t("context.switchModel") }}
        </button>
        <button
          type="button"
          class="btn btn-primary"
          data-test="context-meter-compact"
          :disabled="session.compacting"
          @click="onCompactClick"
        >
          {{ session.compacting ? t("context.compactInProgress") : t("context.compactNow") }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.context-meter {
  position: relative;
  display: flex;
  flex-direction: column;
  padding: 6px 16px;
  border-bottom: 1px solid var(--app-border-color, #d7d7d7);
  background: var(--app-card-color);
}
.empty {
  display: flex;
  align-items: center;
  gap: 8px;
}
.empty-bar {
  display: inline-block;
  height: 6px;
  width: 80px;
  border-radius: 3px;
  background: color-mix(in srgb, var(--app-text-color) 10%, transparent);
}
.empty-label {
  font-size: 12px;
  opacity: 0.6;
}
.meter-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.bar {
  flex: 1;
  display: flex;
  height: 6px;
  border-radius: 3px;
  overflow: hidden;
  background: color-mix(in srgb, var(--app-text-color) 8%, transparent);
  border: none;
  padding: 0;
  cursor: pointer;
}
.segment {
  height: 100%;
  display: block;
}
.numbers {
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  opacity: 0.85;
  white-space: nowrap;
}
.badge {
  font-size: 11px;
  padding: 2px 6px;
  border-radius: 3px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.badge-warn {
  background: color-mix(in srgb, var(--app-warning-color, #faad14) 15%, transparent);
  color: var(--app-warning-color, #faad14);
}
.badge-err {
  background: color-mix(in srgb, var(--app-error-color, #d03050) 15%, transparent);
  color: var(--app-error-color, #d03050);
}
.badge-busy {
  background: color-mix(in srgb, var(--app-primary-color) 15%, transparent);
  color: var(--app-primary-color);
}
.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  animation: pulse 1s ease-in-out infinite;
}
@keyframes pulse {
  50% {
    opacity: 0.3;
  }
}
.popover {
  position: absolute;
  top: 100%;
  left: 16px;
  right: 16px;
  z-index: 20;
  margin-top: 4px;
  background: var(--app-card-color);
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  padding: 8px 12px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.12);
}
.popover-header {
  font-weight: 600;
  font-size: 13px;
  margin-bottom: 6px;
}
.popover-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.popover-table td {
  padding: 3px 0;
}
.popover-table td + td {
  text-align: right;
}
.swatch {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 2px;
  margin-right: 6px;
  vertical-align: middle;
}
.popover-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
.btn {
  padding: 4px 10px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  background: var(--app-card-color);
  color: var(--app-text-color);
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.btn-primary {
  background: var(--app-primary-color);
  color: var(--app-inverse-text-color, #fff);
  border-color: var(--app-primary-color);
}
.btn-ghost {
  background: transparent;
}
</style>
```

> **Verified facts:**
>
> - `useToast()` (in `apps/agent-gui/src/composables/useToast.ts`) returns `{ success, error, info, warning }` — each takes `(message: string, duration?: number)`. Use `toast.error(msg)`, NOT `toast.show("error", msg)`.
> - `useI18n` is auto-imported inside `.vue` SFCs (whitelist in `vite.config.ts`).
> - `ContextSource` is re-exported from `@/types` after Task 5 Step 3.
> - `mountWithPlugins` lives at `apps/agent-gui/src/test-utils/mount.ts` and registers pinia + i18n + a memory-history router.
> - Project Playwright config (`apps/agent-gui/playwright.config.ts`) sets `testIdAttribute: "data-test"` and `baseURL: "http://localhost:1420"`. Tests can use either `getByTestId("name")` or `[data-test="name"]` selectors.
> - `unplugin-vue-components` auto-registers anything under `apps/agent-gui/src/components/**/*.vue`. `<ContextMeter />` works in templates without an explicit import.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd apps/agent-gui && pnpm exec vitest run src/components/__tests__/ContextMeter.test.ts`
Expected: PASS for all six tests.

- [ ] **Step 5: Lint the new file**

Run: `cd apps/agent-gui && pnpm exec oxlint src/components/ContextMeter.vue`
Expected: clean.

Run: `cd apps/agent-gui && pnpm exec stylelint 'src/components/ContextMeter.vue'`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add apps/agent-gui/src/components/ContextMeter.vue apps/agent-gui/src/components/__tests__/ContextMeter.test.ts
git commit -m "feat(gui): add ContextMeter.vue with segmented bar + popover"
```

---

### Task 8 — Mount `<ContextMeter />` in `ChatPanel.vue` + E2E mock updates

**Files:**

- Modify: `apps/agent-gui/src/components/ChatPanel.vue`
- Modify: `apps/agent-gui/e2e/tauri-mock.js`

- [ ] **Step 1: Write the failing test (E2E spec)**

Create `apps/agent-gui/e2e/context-meter.spec.ts`:

```typescript
import { test, expect } from "@playwright/test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const mockScript = readFileSync(join(__dirname, "tauri-mock.js"), "utf8");

test.describe("ContextMeter (P3)", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript({ content: mockScript });
    await page.goto("/");
    // Wait for the workbench to render after auto-init.
    await page.waitForSelector('[data-test="chat-panel"]');
  });

  test("renders the meter after a session has assembled context", async ({ page }) => {
    await expect(page.locator('[data-test="context-meter"]')).toBeVisible();
    // The mock seeds a ContextAssembled on session start.
    await expect(page.locator('[data-test="context-meter-bar"]')).toBeVisible();
  });

  test("opens the popover and triggers compaction", async ({ page }) => {
    await page.click('[data-test="context-meter-bar"]');
    await expect(page.locator('[data-test="context-meter-popover"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-reserved"]')).toBeVisible();

    await page.click('[data-test="context-meter-compact"]');
    // The mock fires Started → Completed. The button should be temporarily disabled.
    await expect(page.locator('[data-test="context-meter-badge-busy"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-badge-busy"]')).toBeHidden({
      timeout: 2_000
    });
  });
});
```

- [ ] **Step 2: Run the failing test**

Run: `cd apps/agent-gui && pnpm exec playwright test context-meter.spec.ts --reporter=list`
Expected: FAIL — `[data-test="context-meter"]` not present (component not mounted).

- [ ] **Step 3: Mount the component in `ChatPanel.vue`**

> **Verified fact:** `apps/agent-gui/src/components/ChatPanel.vue` line 97 reads `<section class="chat-panel" data-test="chat-panel">` followed by `<header class="chat-header">` on line 98. Inject `<ContextMeter />` BETWEEN them as the first child of the section.

Use `file_replace` to insert `<ContextMeter />`:

- `old_string`:
  ```
  <section class="chat-panel" data-test="chat-panel">
      <header class="chat-header">
  ```
- `new_string`:
  ```
  <section class="chat-panel" data-test="chat-panel">
      <ContextMeter />
      <header class="chat-header">
  ```

> Auto-import via `unplugin-vue-components` registers `<ContextMeter />` automatically — no `<script>` import needed.

- [ ] **Step 4: Add the `compact_session` mock handler**

> **Verified facts about `apps/agent-gui/e2e/tauri-mock.js`:**
>
> - `start_session` case (line ~283) creates the session and emits `SessionInitialized` only — no `ContextAssembled` here.
> - `send_message` case (line ~313) DOES emit `ContextAssembled` inside the `setTimeout` (line ~328 `var ctxEvent = makeEvent(sessionId, { type: "ContextAssembled", … })`). So the meter renders the first time the user sends a message.
> - `makeEvent(sessionId, payload)` is the canonical factory.
> - `state.currentSessionId` is set on `start_session` and `switch_session`.
>
> Decision: do NOT emit `ContextAssembled` from `start_session` (the existing UX is "context appears after first message"). The Playwright spec must therefore send a message before asserting the bar is visible.

Find the `case "cancel_session":` block in `apps/agent-gui/e2e/tauri-mock.js` (line ~515). Insert the new `compact_session` handler immediately AFTER its closing brace (`return Promise.resolve(undefined); }` followed by a blank line):

```javascript
case "compact_session": {
  var sid = state.currentSessionId;
  if (!sid) return Promise.reject(new Error("No active session"));
  var startedEvent = makeEvent(sid, {
    type: "ContextCompactionStarted",
    reason: { type: "UserRequested" },
    before_tokens: 12000,
    candidate_event_count: 4
  });
  getTrace(sid).push(startedEvent);
  emitEvent("session-event", startedEvent);
  setTimeout(function () {
    var summaryEvent = makeEvent(sid, {
      type: "CompactionSummary",
      summary_id: "sum_mock_1",
      content: "## User goal\nMock summary content for E2E.",
      replaces_event_range: [new Date().toISOString(), new Date().toISOString()],
      reason: { type: "UserRequested" },
      before_tokens: 12000,
      after_tokens: 3000,
      summarised_by_profile: state.currentProfile
    });
    getTrace(sid).push(summaryEvent);
    emitEvent("session-event", summaryEvent);
    var completedEvent = makeEvent(sid, {
      type: "ContextCompactionCompleted",
      summary_id: "sum_mock_1",
      after_tokens: 3000,
      fallback_used: false
    });
    getTrace(sid).push(completedEvent);
    emitEvent("session-event", completedEvent);
  }, 100);
  return Promise.resolve(undefined);
}
```

Also add `list_profiles_with_limits` AFTER the `list_profiles` case (find it via `grep -n '"list_profiles"' apps/agent-gui/e2e/tauri-mock.js`):

```javascript
case "list_profiles_with_limits": {
  return Promise.resolve(state.profiles.map(function (p) {
    var window;
    var output;
    if (p.alias === "fast") { window = 128000; output = 16384; }
    else if (p.alias === "smart") { window = 200000; output = 16384; }
    else { window = 4096; output = 2048; }
    return {
      alias: p.alias,
      provider: p.provider,
      model_id: p.model_id,
      context_window: window,
      output_limit: output,
      limit_source: "builtin_registry",
      has_api_key: p.has_api_key
    };
  }));
}
```

- [ ] **Step 5: Update the Playwright spec to send a message first**

> **Decision recap:** the existing mock only emits `ContextAssembled` after a `send_message`. The spec must therefore send a message before asserting the meter renders.

Replace the contents of `apps/agent-gui/e2e/context-meter.spec.ts` with:

```typescript
import { test, expect } from "@playwright/test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const mockScript = readFileSync(join(__dirname, "tauri-mock.js"), "utf8");

test.describe("ContextMeter (P3)", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript({ content: mockScript });
    await page.goto("/");
    await page.waitForSelector('[data-test="chat-panel"]');
    // Trigger the existing send_message → ContextAssembled flow in the mock.
    await page.fill('[data-test="message-input"]', "hello from e2e");
    await page.click('[data-test="send-button"]');
    // Wait until ContextAssembled has been applied (bar becomes visible).
    await page.waitForSelector('[data-test="context-meter-bar"]', { timeout: 5_000 });
  });

  test("renders the meter after the first message", async ({ page }) => {
    await expect(page.locator('[data-test="context-meter"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-bar"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-numbers"]')).toContainText("/");
  });

  test("opens the popover and triggers compaction", async ({ page }) => {
    await page.click('[data-test="context-meter-bar"]');
    await expect(page.locator('[data-test="context-meter-popover"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-reserved"]')).toBeVisible();

    await page.click('[data-test="context-meter-compact"]');
    await expect(page.locator('[data-test="context-meter-badge-busy"]')).toBeVisible();
    await expect(page.locator('[data-test="context-meter-badge-busy"]')).toBeHidden({
      timeout: 2_000
    });
  });
});
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd apps/agent-gui && pnpm exec playwright test context-meter.spec.ts --reporter=list`
Expected: PASS for both specs.

Run: `cd apps/agent-gui && pnpm exec vitest run`
Expected: all green (no other component tests broken).

- [ ] **Step 7: Commit**

```bash
git add apps/agent-gui/src/components/ChatPanel.vue apps/agent-gui/e2e/tauri-mock.js apps/agent-gui/e2e/context-meter.spec.ts
git commit -m "feat(gui): mount ContextMeter into ChatPanel + E2E mock + Playwright spec"
```

---

### Task 9 — TUI `status_bar.rs` extend with context line

**Files:**

- Modify: `crates/agent-tui/src/components/mod.rs` (extend `StatusInfo`)
- Modify: `crates/agent-tui/src/components/status_bar.rs` (extend renderer + add `render_context_line`)
- Modify: `crates/agent-tui/src/app.rs` (handle four context events + update `sync_status_bar`)

> **Verified facts (read before editing):**
>
> - `StatusBar` already exists in `crates/agent-tui/src/components/status_bar.rs` (342 lines). It holds a private `info: StatusInfo` and renders via `render_status_bar(area, frame, &info)`.
> - `StatusInfo` is defined in `crates/agent-tui/src/components/mod.rs` (around line 90, just before the `CrossPanelEffect` enum). Current fields: `profile`, `permission_mode`, `session_count`, `mcp_server_count`, `hint`, `error: Option<String>`.
> - `StatusBar` reacts to `CrossPanelEffect::SetStatus(StatusInfo)`, populated by `App::sync_status_bar` in `crates/agent-tui/src/app.rs`.
> - `agent_core::context_types::{ContextSource, ContextUsage}` are the exported types (P1 work).
> - `App::handle_domain_event` is where we hook the four context events.
> - Bottom row in `App::render` is a single `Constraint::Length(1)` row. To preserve layout we keep ONE row; the new `render_context_line` must be one line long.

- [ ] **Step 1: Extend `StatusInfo` with the context fields**

In `crates/agent-tui/src/components/mod.rs`, locate the `pub struct StatusInfo { … }` definition (just above `pub enum CrossPanelEffect`). Add three new fields at the end of the struct:

```rust
/// Last assembled context usage snapshot. None until first ContextAssembled.
pub context_usage: Option<agent_core::context_types::ContextUsage>,
/// True while a ContextCompactionStarted has been seen but Completed/Failed has not.
pub compacting: bool,
```

Update every literal `StatusInfo { … }` constructor (use `grep -rn 'StatusInfo {' crates/agent-tui/src/ | cat` to find them — at minimum: `status_bar.rs::StatusBar::new`, the existing tests in `status_bar.rs`, and `app.rs::sync_status_bar`) so each one initialises the two new fields with `context_usage: None,` and `compacting: false,`.

- [ ] **Step 2: Write the failing tests**

Append a new module to `crates/agent-tui/src/components/status_bar.rs` AFTER the existing `#[cfg(test)] mod tests` block:

```rust
#[cfg(test)]
mod context_line_tests {
    use super::*;
    use agent_core::context_types::{ContextSource, ContextUsage};

    fn usage(total: u64, budget: u64) -> ContextUsage {
        ContextUsage {
            total_tokens: total,
            budget_tokens: budget,
            context_window: budget + 20_000,
            output_reservation: 20_000,
            by_source: vec![
                (ContextSource::System, 2_000),
                (ContextSource::ToolDefinitions, 22_000),
                (ContextSource::Memory, 9_000),
                (ContextSource::History, 64_000),
                (ContextSource::ToolResult, 13_000),
            ],
            estimator: "cl100k_base",
            corrected_by_real_usage: false,
        }
    }

    fn make_info(usage_opt: Option<ContextUsage>, compacting: bool) -> StatusInfo {
        StatusInfo {
            profile: "fast".into(),
            permission_mode: "suggest".into(),
            session_count: 1,
            mcp_server_count: 0,
            hint: String::new(),
            error: None,
            context_usage: usage_opt,
            compacting,
        }
    }

    #[test]
    fn render_context_line_long_form_under_wide_terminal() {
        let info = make_info(Some(usage(110_000, 180_000)), false);
        let rendered = render_context_line_string(&info, 120);
        assert!(rendered.contains("profile: fast"), "got: {rendered}");
        assert!(rendered.contains("perm: suggest"), "got: {rendered}");
        assert!(rendered.contains("ctx: 110.0k/180.0k"), "got: {rendered}");
        assert!(rendered.contains("sys 2k"), "got: {rendered}");
        assert!(rendered.contains("hist 64k"), "got: {rendered}");
    }

    #[test]
    fn render_context_line_short_form_under_narrow_terminal() {
        let info = make_info(Some(usage(152_000, 200_000)), false);
        let rendered = render_context_line_string(&info, 60);
        assert!(rendered.contains("ctx: 152.0k/200.0k"), "got: {rendered}");
        assert!(rendered.contains("(76%)"), "got: {rendered}");
        // Short form does NOT include per-source breakdown
        assert!(!rendered.contains("sys 2k"), "got: {rendered}");
    }

    #[test]
    fn render_context_line_warn_glyph_at_70_pct() {
        let info = make_info(Some(usage(140_000, 180_000)), false); // ≈78%
        let rendered = render_context_line_string(&info, 60);
        assert!(rendered.contains('⚠'), "got: {rendered}");
    }

    #[test]
    fn render_context_line_shows_compacting_indicator() {
        let info = make_info(Some(usage(50_000, 180_000)), true);
        let rendered = render_context_line_string(&info, 120);
        assert!(rendered.contains("compacting"), "got: {rendered}");
    }

    #[test]
    fn render_context_line_handles_no_usage_gracefully() {
        let info = make_info(None, false);
        let rendered = render_context_line_string(&info, 120);
        assert!(rendered.contains("profile: fast"), "got: {rendered}");
        assert!(rendered.contains("ctx: -"), "got: {rendered}");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p agent-tui context_line_tests`
Expected: FAIL — `render_context_line_string` undefined.

- [ ] **Step 4: Extend `StatusInfo` with two new fields**

> **Verified facts** (gathered with `grep -n 'pub struct StatusInfo\|StatusInfo {' crates/agent-tui/src/components/mod.rs crates/agent-tui/src/components/status_bar.rs crates/agent-tui/src/app.rs | cat`):
>
> - `pub struct StatusInfo` is defined in `crates/agent-tui/src/components/mod.rs:84-91` with exactly six fields: `profile: String`, `permission_mode: String`, `session_count: usize`, `mcp_server_count: usize`, `hint: String`, `error: Option<String>`.
> - Literal `StatusInfo { … }` constructors exist in **7 places** that must all be updated:
>   - `crates/agent-tui/src/components/status_bar.rs:64` (inside `StatusBar::new`)
>   - `crates/agent-tui/src/components/status_bar.rs:211, 230, 249, 280, 321` (inside `#[cfg(test)] mod tests`)
>   - `crates/agent-tui/src/app.rs:712` (inside `App::sync_status_bar`)

In `crates/agent-tui/src/components/mod.rs`, replace lines 83-91 (the entire `StatusInfo` struct) with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusInfo {
    pub profile: String,
    pub permission_mode: String,
    pub session_count: usize,
    pub mcp_server_count: usize,
    pub hint: String,
    pub error: Option<String>,
    /// P3: latest `ContextAssembled.usage`. `None` until the first event.
    pub context_usage: Option<agent_core::context_types::ContextUsage>,
    /// P3: `true` between `ContextCompactionStarted` and `Completed`/`Failed`.
    pub compacting: bool,
}
```

Then update **every one of the 7 literal constructors** to include `context_usage` and `compacting`. The 6 in-test/in-`new` constructors all use defaults; only `app.rs::sync_status_bar` reads from `App` state. **Run Step 8 BEFORE making the `app.rs:712` edit** so `self.last_context_usage` and `self.compacting` exist when this `StatusInfo` literal references them; otherwise the project will not compile between Step 4 and Step 8.

**Edit 1 — `crates/agent-tui/src/components/status_bar.rs:62-72` (`StatusBar::new`):**

Replace the existing `StatusInfo { … }` literal in `StatusBar::new` with the full 8-field form:

```rust
            info: StatusInfo {
                profile: String::new(),
                permission_mode: String::new(),
                session_count: 0,
                mcp_server_count: 0,
                hint: String::new(),
                error: None,
                context_usage: None,
                compacting: false,
            },
```

**Edit 2 — `crates/agent-tui/src/components/status_bar.rs:211, 230, 249, 280, 321` (5 test constructors inside `#[cfg(test)] mod tests`):**

Each existing constructor has the same 6-field shape:

```rust
        let info = StatusInfo {
            profile: "fast".into(),
            permission_mode: "suggest".into(),
            session_count: 3,
            mcp_server_count: 0,
            hint: "Alt+Q quit".into(),
            error: None,
        };
```

Append the two new lines before the closing `}`:

```rust
            context_usage: None,
            compacting: false,
        };
```

**Edit 3 — `crates/agent-tui/src/app.rs:712-719` (`App::sync_status_bar`):**

(Defer this until after Step 8 has added `App::last_context_usage` and `App::compacting`.) Replace the existing 6-field literal with:

```rust
        let info = crate::components::StatusInfo {
            profile: self.state.model_profile.clone(),
            permission_mode: self.state.permission_mode.as_str().to_string(),
            session_count: self.state.sessions.len(),
            mcp_server_count: 0,
            hint,
            error: None,
            context_usage: self.last_context_usage.clone(),
            compacting: self.compacting,
        };
```

- [ ] **Step 5: Implement `render_context_line_string` and the helpers**

In `crates/agent-tui/src/components/status_bar.rs`, add to the imports at the top of the file (after the existing `use super::{Command, …}` line, since `super::` already brings `StatusInfo` into scope from `mod.rs`):

```rust
use agent_core::context_types::{ContextSource, ContextUsage};
```

Then insert the following block immediately ABOVE the `// ---------------------------------------------------------------------------\n// Tests\n// ---------------------------------------------------------------------------` separator (currently around line 246, right after `render_status_bar` ends):

```rust
// ---------------------------------------------------------------------------
// P3: Context-meter status line
// ---------------------------------------------------------------------------

/// Format a token count as `1.2k` for >=1000, otherwise the raw number.
fn fmt_tokens(n: u64) -> String {
    if n >= 1_000 {
        format!("{:.1}k", (n as f64) / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Compact form of `fmt_tokens` for per-source breakdown chips: `12k` (no decimal).
fn fmt_short(n: u64) -> String {
    if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

/// Map a `ContextSource` to a 3-5 char chip label for the breakdown line.
fn source_short_label(source: &ContextSource) -> &'static str {
    match source {
        ContextSource::System => "sys",
        ContextSource::ToolDefinitions => "tools",
        ContextSource::Request => "req",
        ContextSource::Memory => "mem",
        ContextSource::History => "hist",
        ContextSource::ToolResult => "tres",
        ContextSource::SelectedFile => "file",
        ContextSource::CompactionSummary => "csum",
    }
}

/// Render a single status line including the context-meter info. Returns a
/// plain `String` so unit tests can assert on the human-readable form
/// without going through ratatui rendering.
///
/// Layout:
/// - Always: `profile: <name>  perm: <mode>`
/// - When `usage.is_some()`:
///   - `width >= 100`: long form `ctx: <tot>/<bud>[ ⚠]  <chip1> <n1> <chip2> <n2> …`
///   - `width <  100`: short form `ctx: <tot>/<bud> (<pct>%)[ ⚠]`
/// - When `usage.is_none()`: `ctx: -`
/// - When `compacting`: appends `compacting…`
pub fn render_context_line_string(info: &StatusInfo, width: u16) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!("profile: {}", info.profile));
    parts.push(format!("perm: {}", info.permission_mode));

    match &info.context_usage {
        Some(u) => {
            let pct = if u.budget_tokens == 0 {
                0
            } else {
                (((u.total_tokens as f64) / (u.budget_tokens as f64)) * 100.0).round() as u64
            };
            // Warning glyph at >=70%; the spec uses the same glyph for >=85%
            // because the GUI surfaces an additional badge for the err tier.
            let warn = if pct >= 70 { " ⚠" } else { "" };

            if width >= 100 {
                parts.push(format!(
                    "ctx: {}/{}{}",
                    fmt_tokens(u.total_tokens),
                    fmt_tokens(u.budget_tokens),
                    warn
                ));
                let mut breakdown = String::new();
                for (source, tokens) in &u.by_source {
                    breakdown.push_str(&format!(
                        " {} {}",
                        source_short_label(source),
                        fmt_short(*tokens)
                    ));
                }
                parts.push(breakdown.trim_start().to_string());
            } else {
                parts.push(format!(
                    "ctx: {}/{} ({}%){}",
                    fmt_tokens(u.total_tokens),
                    fmt_tokens(u.budget_tokens),
                    pct,
                    warn
                ));
            }
        }
        None => parts.push("ctx: -".into()),
    }

    if info.compacting {
        parts.push("compacting…".into());
    }

    parts.join("  ")
}

```

> **Why `ContextUsage` is in the import list even though `render_context_line_string` only takes `&StatusInfo`:** `StatusInfo.context_usage: Option<ContextUsage>` already brings the type into scope through the field declaration in `mod.rs`, but importing it explicitly here keeps the rustdoc cross-reference (`/// …based on [`ContextUsage`]…` if you choose to add one later) resolvable without a fully-qualified path. If `cargo clippy -- -D warnings` complains about an unused import, drop `ContextUsage` from the `use` line and keep only `ContextSource`. Both forms are correct.

    let mut parts: Vec<String> = Vec::new();
    parts.push(format!("profile: {}", info.profile));
    parts.push(format!("perm: {}", info.permission_mode));

    match &info.context_usage {
        Some(u) => {
            let pct = if u.budget_tokens == 0 {
                0
            } else {
                (((u.total_tokens as f64) / (u.budget_tokens as f64)) * 100.0).round() as u64
            };
            let warn = if pct >= 70 { " ⚠" } else { "" };

            if width >= 100 {
                parts.push(format!(
                    "ctx: {}/{}{}",
                    fmt_tokens(u.total_tokens),
                    fmt_tokens(u.budget_tokens),
                    warn
                ));
                let mut breakdown = String::new();
                for (source, tokens) in &u.by_source {
                    breakdown.push_str(&format!(
                        " {} {}",
                        source_short_label(source),
                        fmt_short(*tokens)
                    ));
                }
                parts.push(breakdown.trim_start().to_string());
            } else {
                parts.push(format!(
                    "ctx: {}/{} ({}%){}",
                    fmt_tokens(u.total_tokens),
                    fmt_tokens(u.budget_tokens),
                    pct,
                    warn
                ));
            }
        }
        None => parts.push("ctx: -".into()),
    }

    if info.compacting {
        parts.push("compacting…".into());
    }

    parts.join("  ")

}

````

- [ ] **Step 6: Switch `render_status_bar` to use the context line when usage is present**

> **Verified fact:** `pub fn render_status_bar(area: Rect, frame: &mut Frame, info: &StatusInfo)` is defined at `crates/agent-tui/src/components/status_bar.rs:129` and its body builds a `Vec<Span>` (profile badge → permission badge → session count → MCP count → hint → optional error) before calling `frame.render_widget(Paragraph::new(Line::from(spans)), area)` at the end (around line 244).

Modify `render_status_bar` by inserting THIS block as the very first statement inside the function body (immediately after the `{` on line 129, before the existing `let mut spans: Vec<Span> = Vec::new();`):

```rust
    // P3: when we have observed at least one ContextAssembled event, switch
    // to the dedicated context-meter line. The legacy renderer below remains
    // the fallback for the cold-start case (no usage yet).
    if info.context_usage.is_some() {
        let line_text = render_context_line_string(info, area.width);
        frame.render_widget(Paragraph::new(Line::from(Span::raw(line_text))), area);
        return;
    }
````

Do NOT modify any other line of `render_status_bar` — the existing 6-field span-building body remains unchanged and serves as the fallback when `info.context_usage.is_none()`.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p agent-tui context_line_tests`
Expected: PASS for all five tests.

Run: `cargo test -p agent-tui`
Expected: all green. The existing `mod tests` block in `status_bar.rs` builds `StatusInfo` literals with `context_usage: None` (per the Step 4 update), so the legacy span-building path is exercised and its assertions still hold.

- [ ] **Step 8: Add two fields to `App` and wire the four context events**

> **Verified facts** (from `grep -n 'pub struct App\b\|fn handle_domain_event\|fn sync_status_bar\|impl App' crates/agent-tui/src/app.rs | cat`):
>
> - `pub struct App` is declared in `crates/agent-tui/src/app.rs` (run the grep above to get the exact line; for the current snapshot it is the only `pub struct App` in the file).
> - `App::new` is the sole constructor — find it via `grep -n 'impl App\|pub fn new' crates/agent-tui/src/app.rs | cat`.
> - `pub fn handle_domain_event(&mut self, event: &DomainEvent)` is at `crates/agent-tui/src/app.rs:293` and dispatches on `event.payload` (an `EventPayload`). The catch-all is `_ => {}` near the end of that match.
> - `App::sync_status_bar` is at `crates/agent-tui/src/app.rs:705-722` and builds a `StatusInfo { profile, permission_mode, session_count, mcp_server_count, hint, error: None }` literal at line 712, then sends it via `self.status_bar.handle_effect(&CrossPanelEffect::SetStatus(info))`.

Make three edits to `crates/agent-tui/src/app.rs`:

**(a)** Add two fields to the `App` struct declaration:

```rust
    /// P3: latest `ContextAssembled.usage`, propagated into the status bar.
    last_context_usage: Option<agent_core::context_types::ContextUsage>,
    /// P3: `true` between `ContextCompactionStarted` and `Completed`/`Failed`.
    compacting: bool,
```

**(b)** Initialise both fields in `App::new`. Find the existing `Self { … }` literal in the constructor and append:

```rust
            last_context_usage: None,
            compacting: false,
```

**(c)** In `handle_domain_event`, add four arms to the `match event.payload { … }` block immediately BEFORE the catch-all `_ => {}`:

```rust
            agent_core::EventPayload::ContextAssembled { usage } => {
                self.last_context_usage = Some(usage.clone());
                self.compacting = false;
                self.sync_status_bar();
                self.state.render_scheduler.mark_dirty();
            }
            agent_core::EventPayload::ContextCompactionStarted { .. } => {
                self.compacting = true;
                self.sync_status_bar();
                self.state.render_scheduler.mark_dirty();
            }
            agent_core::EventPayload::ContextCompactionCompleted { .. } => {
                self.compacting = false;
                self.sync_status_bar();
                self.state.render_scheduler.mark_dirty();
            }
            agent_core::EventPayload::ContextCompactionFailed { .. } => {
                self.compacting = false;
                self.sync_status_bar();
                self.state.render_scheduler.mark_dirty();
            }
```

**(d)** Modify the `StatusInfo` literal at `app.rs:712-719` (inside `sync_status_bar`) — the existing 6 fields are kept verbatim; we only ADD the two new lines before the closing `}`. The full literal becomes:

```rust
        let info = crate::components::StatusInfo {
            profile: self.state.model_profile.clone(),
            permission_mode: self.state.permission_mode.as_str().to_string(),
            session_count: self.state.sessions.len(),
            mcp_server_count: 0,
            hint,
            error: None,
            context_usage: self.last_context_usage.clone(),
            compacting: self.compacting,
        };
```

- [ ] **Step 9: Verify all TUI tests still pass**

Run: `cargo test -p agent-tui`
Expected: all green. Test counts:

- `crates/agent-tui/src/components/status_bar.rs` `mod tests`: previously 5, still 5 (the 6 literal constructors at lines 211/230/249/280/321 + the `StatusBar::new` at line 64 were updated in Step 4 to include `context_usage: None, compacting: false`).
- `crates/agent-tui/src/components/status_bar.rs` `mod context_line_tests`: 5 new (added in Step 2).
- `crates/agent-tui/tests/app_logic.rs`: existing 7 still pass.

If the `app_logic.rs` tests construct `App` directly via a helper, that helper transitively goes through `App::new` which now sets both new fields — no helper change required.

- [ ] **Step 10: Commit**

```bash
git add crates/agent-tui/src/components/mod.rs crates/agent-tui/src/components/status_bar.rs crates/agent-tui/src/app.rs
git commit -m "feat(tui): render context-usage line in StatusBar with per-source breakdown"
```

---

### Task 10 — TUI `:compact` command parsing + runtime dispatch

**Files:**

- Modify: `crates/agent-tui/src/components/mod.rs` (extend `enum Command`)
- Modify: `crates/agent-tui/src/components/chat.rs` (intercept `:compact` in `apply_key_action`)
- Modify: `crates/agent-tui/src/main.rs` (add dispatcher arm in `dispatch_commands`)
- Modify: `crates/agent-tui/tests/app_logic.rs` (new test)

> **Verified facts (cross-checked with grep before this task):**
>
> - `pub enum Command` is declared in `crates/agent-tui/src/components/mod.rs` line 108. Variants today: `SendMessage`, `DecidePermission`, `TrustMcpServer`, `CancelSession`, `StartSession`, `SwitchSession`. All `#[derive(Debug, Clone, PartialEq, Eq)]` with `#[allow(dead_code)]`.
> - The dispatcher is `async fn dispatch_commands` in `crates/agent-tui/src/main.rs` line 46. It takes `runtime: &Arc<LocalRuntime<…>>`, `app: &mut App`, `commands: Vec<Command>` and `match`-es each `Command`. The existing `Command::CancelSession` arm (line 124) shows the canonical pattern: `runtime.cancel_session(workspace_id, session_id).await`. Mirror it.
> - `EventContext` (in `crates/agent-tui/src/components/mod.rs` ~line 145) has `pub workspace_id: &'a agent_core::WorkspaceId` and `pub current_session_id: &'a Option<agent_core::SessionId>`.
> - `ChatPanel::apply_key_action` is at `crates/agent-tui/src/components/chat.rs` line 50. The `KeyAction::SendInput` arm (line 60) is gated on `if !self.input_content.is_empty()` — this means a `:compact`-only buffer would normally produce `SendMessage`. We must intercept BEFORE the existing arm matches.
> - `LocalRuntime::compact_session` exists at `crates/agent-runtime/src/facade_runtime.rs:358`. Its signature: `pub async fn compact_session(&self, session_id: SessionId, reason: CompactionReason) -> Result<...>`.
> - `agent_core::CompactionReason::UserRequested` is the variant for explicit user invocations (P2 added this).

- [ ] **Step 1: Add the `CompactSession` variant**

In `crates/agent-tui/src/components/mod.rs`, locate `pub enum Command` (line 108). After the existing `SwitchSession { session_id: SessionId },` variant (the last one, around line 137), add:

```rust
CompactSession {
    workspace_id: agent_core::WorkspaceId,
    session_id: agent_core::SessionId,
},
```

- [ ] **Step 2: Write the failing test**

Append to `crates/agent-tui/tests/app_logic.rs`:

```rust
// ---------------------------------------------------------------------------
// Test: `:compact` command intercepted by ChatPanel
// ---------------------------------------------------------------------------

#[test]
fn colon_compact_input_dispatches_compact_session_command() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tools::PermissionMode;
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();

    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":compact".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::CompactSession { .. })),
        "expected Command::CompactSession; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
    // Buffer should be cleared after `:compact` is consumed.
    assert!(
        chat.input_content.is_empty(),
        "expected input cleared, got {:?}",
        chat.input_content
    );
}
```

> **Verified fact** (from `cat crates/agent-tui/src/lib.rs`): the TUI crate exposes `pub mod app; pub mod app_state; pub mod components; pub mod keybindings; pub mod view;` from `crates/agent-tui/src/lib.rs`. The integration test imports above (`agent_tui::components::chat::ChatPanel`, `agent_tui::components::{Command, EventContext, FocusTarget}`, `agent_tui::keybindings::KeyAction`) all resolve through this `lib.rs`. No additional `pub use` re-exports are required.

- [ ] **Step 3: Run the failing test**

Run: `cargo test -p agent-tui colon_compact_input_dispatches_compact_session_command`
Expected: FAIL — `Command::CompactSession` variant not yet matched in chat (or won't compile if Step 1 succeeded but Step 4 hasn't run).

- [ ] **Step 4: Intercept `:compact` in `ChatPanel::apply_key_action`**

In `crates/agent-tui/src/components/chat.rs`, modify the `KeyAction::SendInput` arm (currently line 60-72). The current arm is:

```rust
KeyAction::SendInput if !self.input_content.is_empty() => {
    self.input_history.push(self.input_content.clone());
    let content = std::mem::take(&mut self.input_content);
    self.input_cursor = 0;
    self.input_history_index = None;

    if let Some(session_id) = ctx.current_session_id {
        commands.push(Command::SendMessage {
            workspace_id: ctx.workspace_id.clone(),
            session_id: session_id.clone(),
            content,
        });
    }
}
```

Replace it with:

```rust
KeyAction::SendInput if !self.input_content.is_empty() => {
    let trimmed = self.input_content.trim();
    // P3: intercept ":compact" before treating the input as a chat message.
    if trimmed == ":compact" {
        self.input_content.clear();
        self.input_cursor = 0;
        self.input_history_index = None;
        if let Some(session_id) = ctx.current_session_id {
            commands.push(Command::CompactSession {
                workspace_id: ctx.workspace_id.clone(),
                session_id: session_id.clone(),
            });
        }
    } else {
        self.input_history.push(self.input_content.clone());
        let content = std::mem::take(&mut self.input_content);
        self.input_cursor = 0;
        self.input_history_index = None;

        if let Some(session_id) = ctx.current_session_id {
            commands.push(Command::SendMessage {
                workspace_id: ctx.workspace_id.clone(),
                session_id: session_id.clone(),
                content,
            });
        }
    }
}
```

- [ ] **Step 5: Wire the dispatcher in `main.rs`**

In `crates/agent-tui/src/main.rs`, find the `Command::CancelSession { workspace_id, session_id } => { … }` arm in `dispatch_commands` (line ~124). Immediately AFTER its closing brace, add:

```rust
Command::CompactSession {
    workspace_id: _,
    session_id,
} => {
    if let Err(e) = runtime
        .compact_session(session_id, agent_core::CompactionReason::UserRequested)
        .await
    {
        app.state.current_session.messages.push(
            agent_core::projection::ProjectedMessage {
                role: agent_core::projection::ProjectedRole::Assistant,
                content: format!("[compact error: {e}]"),
            },
        );
        app.state.render_scheduler.mark_dirty();
    }
}
```

> The existing `Command::CancelSession` arm shows the canonical pattern (no `tokio::spawn` — `dispatch_commands` is itself an `async fn` invoked from the main loop, so we just `.await` directly).

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p agent-tui colon_compact_input_dispatches_compact_session_command`
Expected: PASS.

Run: `cargo test -p agent-tui`
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add crates/agent-tui/src/components/mod.rs crates/agent-tui/src/components/chat.rs crates/agent-tui/src/main.rs crates/agent-tui/tests/app_logic.rs
git commit -m "feat(tui): add :compact command that dispatches CompactSession to runtime"
```

---

### Task 11 — (REMOVED)

> **Decision:** the original "no-op verification of `useTauriEvents`" task added zero source-file changes — the existing `useTauriEvents.ts` already routes session-filtered events to `session.applyEvent`, which Task 5 extended with the four new context handlers. The redundant assertion duplicated coverage from Task 5's tests. Removed to keep the plan tight; proceed straight to Task 12.

---

### Task 12 — Final verification, generated types sync, push

**Files:** none (verification only)

- [ ] **Step 1: Regenerate specta bindings + verify in sync**

Run: `just gen-types`
Expected: regenerates `apps/agent-gui/src/generated/{commands,events}.ts`.

Run: `just check-types`
Expected: PASS — no uncommitted changes in `apps/agent-gui/src/generated/`. If there ARE diffs, commit them:

```bash
git add apps/agent-gui/src/generated/
git commit -m "chore(gui): refresh specta bindings for P3"
```

- [ ] **Step 2: Run the full Rust test suite**

Run: `cargo test --workspace --all-targets`
Expected: all green.

If anything fails: STOP. This is a blocker — invoke `superpowers:systematic-debugging` to investigate.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: zero warnings.

- [ ] **Step 4: Run the full GUI test suite + lint + format**

Run from the worktree root:

```bash
pnpm run format:check
pnpm run lint
just test-gui
```

Expected: all green.

- [ ] **Step 5: Run the new Playwright spec**

Run: `cd apps/agent-gui && pnpm exec playwright test context-meter.spec.ts --reporter=list`
Expected: PASS for both specs.

- [ ] **Step 6: Smoke-test the GUI in dev mode (manual)**

Run: `just gui-dev` in one terminal. Open `http://localhost:1420/`.

Verify:

1. The `<ContextMeter />` bar appears at the top of the chat panel after a session is active and the mock has emitted `ContextAssembled`.
2. Clicking the bar opens the popover with per-source rows.
3. Clicking **Compact now** toggles the busy badge and resolves to idle.
4. Switching the GUI to dark mode (settings → Theme → Dark) keeps the bar readable.

Stop the dev server (Ctrl+C) when done.

- [ ] **Step 7: Smoke-test the TUI**

Run: `just tui` in a terminal large enough (≥120 cols).

Verify:

1. The bottom status line shows `profile: <name>  perm: <mode>  ctx: <total>/<budget>` after the first message.
2. Typing `:compact` and pressing Enter triggers a `compacting…` indicator (the fake model fires `ContextCompactionCompleted` quickly).
3. Resizing the terminal to <100 cols collapses to short form.

- [ ] **Step 8: Push the branch**

```bash
git push -u origin feat/context-p3-ui-context-meter
```

- [ ] **Step 9: Hand off to `superpowers:finishing-a-development-branch`**

Announce: "I'm using the finishing-a-development-branch skill to complete this work."

Follow that skill: verify tests one more time, present the four standard options (Merge / PR / Keep / Discard), execute the user's choice. Cleanup the worktree only on Options 1 or 4.

---

## Self-Review Checklist (run after Task 12)

**1. Spec coverage** (§4.6 — UI layer):

- ✅ GUI `ContextMeter.vue` segmented bar + status badges + popover + actions row → Tasks 7, 8
- ✅ Pinia store fields (`lastContextUsage`, `modelLimits`, `compacting`, `lastCompactionError`) → Task 5
- ✅ Tauri command `compact_session` → Task 3
- ✅ Tauri command `list_profiles_with_limits` → Task 4
- ⏸️ Tauri command `switch_model` → DEFERRED to P4 (button is rendered but disabled, see Out-of-scope)
- ✅ i18n keys (`context.*`, `status.compacting`, `status.contextNearFull`, `errors.sessionBusy`) → Task 6
- ✅ E2E mock handlers (`compact_session`, `list_profiles_with_limits`) → Task 8 (note: existing `send_message` flow already emits `ContextAssembled`, no extra seeding needed)
- ✅ TUI `status_bar.rs` long + short form per-source breakdown → Task 9
- ✅ TUI `:compact` command parsing + dispatch → Task 10
- ⏸️ TUI `:model <alias>` command → DEFERRED to P4

**2. Placeholder scan**: no `TODO`, no "implement later"; Task 11 explicitly removed. Every step shows complete code derived from verified facts about the actual codebase.

**3. Type consistency**:

- `CompactionStatus` enum: defined in Task 1 with discriminants `Idle | Running | Failed { error }` (`#[serde(tag = "type")]`, `Default = Idle`) — referenced identically in Task 5 (Pinia type re-exports + `applyEvent` arms) and Task 7 (Vue component badge logic).
- `ProjectedModelLimits` struct: defined in Task 1 with fields `{ context_window, output_limit, source: String }` — used in Tasks 5 (Pinia) and surfaced through P4 later.
- `ProfileWithLimits` DTO: defined in Task 4 with fields `{ alias, provider, model_id, context_window, output_limit, limit_source, has_api_key }` — shape stable.
- `ContextSource` enum: imported from existing `agent_core::context_types` (P1 work) — Tasks 7 (Vue) and 9 (TUI) both use the same `PascalCase` discriminants the existing P1/P2 events serialise as.
- `StatusInfo` struct: extended in Task 9 Step 1 with `context_usage: Option<ContextUsage>` and `compacting: bool` — both fields propagated from `App::handle_domain_event` via `App::sync_status_bar`.
- `Command::CompactSession { workspace_id, session_id }`: added in Task 10 Step 1 to the existing `Command` enum at `crates/agent-tui/src/components/mod.rs:108`. Both fields owned types so the dispatcher in `main.rs::dispatch_commands` can directly `await` without lifetime gymnastics.

**4. Skill alignment**:

- TDD followed in Tasks 1-10 (fail-first → minimal impl → green).
- Verification done in Task 12 (the `verification-before-completion` discipline).
- Fresh branch from `main` in an isolated worktree (the `using-git-worktrees` discipline).
- Frequent commits — one per task at minimum.

---

# Model Stream Diagnostics Classification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Export recent model stream status diagnostics and compact final stream timeouts into structured `model_stream_failure` signals.

**Architecture:** Keep the runtime/model stream handler unchanged. `export_session_diagnostics` copies the last few `ModelStreamStatus` events into its DTO; `session-diagnostics-snapshot.mjs` parses only structured key/value fields from those status messages and prioritizes model-stream failure over generic trajectory/task failure.

**Tech Stack:** Rust Tauri commands, Specta type generation, Node test runner.

---

### Task 1: Export Recent Model Stream Statuses

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands/session.rs`
- Modify: `apps/agent-gui/src-tauri/src/bin/export_specta.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Generated: `apps/agent-gui/src/generated/commands.ts`

- [x] **Step 1: Write failing Rust tests**

Add tests in `session_diagnostics_tests` proving `summarize_trace_export` keeps only the last five `ModelStreamStatus` events and preserves `phase`, `retrying`, `retry_attempt`, `max_retries`, and `message`.

- [x] **Step 2: Run the focused Rust test**

Run: `cargo test -p agent-gui-tauri summarize_trace_export_keeps_recent_model_stream_statuses`
Expected before implementation: compile/test failure because `recent_model_stream_statuses` does not exist.

- [x] **Step 3: Implement minimal export DTO**

Add `ModelStreamStatusDiagnosticsResponse`, append `recent_model_stream_statuses` to `SessionDiagnosticsResponse`, collect with a fixed five-entry cap in `summarize_trace_export`, and register the DTO with Specta/export.

- [x] **Step 4: Regenerate bindings**

Run: `just gen-types`
Expected after implementation: `apps/agent-gui/src/generated/commands.ts` includes `ModelStreamStatusDiagnosticsResponse` and `recent_model_stream_statuses`.

### Task 2: Compact Model Stream Failures

**Files:**

- Modify: `scripts/session-diagnostics-snapshot.mjs`
- Modify: `scripts/session-diagnostics-snapshot.test.mjs`

- [x] **Step 1: Write failing Node tests**

Add tests for:

- `stalled_after_progress` when final status has token/tool progress.
- `no_event_timeout` when final status has no token/tool progress.
- retrying-only status does not become `model_stream_failure`.
- compact JSON omits user/assistant text and token delta text.

- [x] **Step 2: Run focused Node tests**

Run: `node --test scripts/session-diagnostics-snapshot.test.mjs --test-name-pattern 'model stream'`
Expected before implementation: failure because `model_stream_failure` is missing.

- [x] **Step 3: Implement minimal parser**

Read `recent_model_stream_statuses`, skip `retrying: true`, parse key/value fields from `message`, return `model_stream_failure`, and make `failure_signal` prefer `model_stream_<kind>`.

- [x] **Step 4: Run required verification**

Run:

- `cargo fmt --all`
- `cargo fmt --all --check`
- `cargo test -p agent-gui-tauri session_diagnostics`
- `node --test scripts/session-diagnostics-snapshot.test.mjs --test-name-pattern 'model stream'`
- `node --test scripts/session-diagnostics-snapshot.test.mjs`
- `just gen-types`
- `bun run format:check`

Dev App validation is not needed because this is diagnostics export/snapshot behavior covered by Rust and Node tests.

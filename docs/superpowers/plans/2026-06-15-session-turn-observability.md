# Session Turn Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make stuck Kairox agent turns observable and controllable enough for automation by fixing empty-response outcomes, explicit session sends, cancellation cleanup, and stream retry events.

**Architecture:** Keep the existing actor queue for normal chat sends, but add an explicit Tauri command for automation that targets a session id and rejects busy sessions. Preserve graceful cancellation while adding a bounded force-abort fallback. Surface stream-start retry/final timeout through session events so monitors do not need terminal logs.

**Tech Stack:** Rust runtime (`agent-runtime`, `agent-core`), Tauri command layer (`agent-gui-tauri`), Specta generated TypeScript bindings, focused Rust tests.

---

### Task 1: Empty Response Fallback Outcome

**Files:**

- Modify: `crates/agent-runtime/src/agent_loop/stream_handler.rs`
- Modify: `crates/agent-runtime/src/agent_loop/runner.rs`
- Test: `crates/agent-runtime/src/agent_loop/stream_handler_tests.rs`

- [ ] **Step 1: Write failing test**
      Add a stream handler test that calls `process_model_stream_with_idle_timeout` with `empty_response_fallback: Some(...)` and an empty stream, then asserts `StreamOutput.empty_response_fallback_used == true`.

- [ ] **Step 2: Verify RED**
      Run: `cargo test -p agent-runtime stream_empty_response_fallback_marks_output -- --exact`
      Expected: compile failure or assertion failure because the flag does not exist.

- [ ] **Step 3: Implement minimal code**
      Add `empty_response_fallback_used: bool` to `StreamOutput`; set it only for the fallback branch. In `runner.rs`, when no tool calls and the flag is true, call `fail_root_task` and complete trajectory as `Failed` instead of success.

- [ ] **Step 4: Verify GREEN**
      Run: `cargo test -p agent-runtime stream_empty_response_fallback_marks_output -- --exact`
      Expected: one test passes.

### Task 2: Explicit Session Send + Busy Gate

**Files:**

- Modify: `crates/agent-runtime/src/facade_session_ops.rs`
- Modify: `crates/agent-runtime/src/facade_runtime/tests/send_message_tests.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands/chat.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Regenerate: `apps/agent-gui/src/generated/commands.ts`

- [ ] **Step 1: Write failing runtime test**
      Add a test that starts a blocking turn, calls a strict send path for the same session, and expects `CoreError::SessionBusy` with a reason mentioning the current execution state.

- [ ] **Step 2: Verify RED**
      Run: `cargo test -p agent-runtime send_message_strict_rejects_running_session -- --exact`
      Expected: compile failure because the strict send path does not exist.

- [ ] **Step 3: Implement runtime strict send**
      Add `send_message_strict` on `LocalRuntime` or equivalent internal helper that checks `SessionExecutionRuntime::session_state` and returns `SessionBusy` for `Running`/`Cancelling`; keep existing `send_message` queueing behavior unchanged.

- [ ] **Step 4: Add Tauri command**
      Add `send_message_to_session(session_id, content, attachments)` that prepares slash-skill content for the provided session and uses the strict send helper. Register it in Tauri invoke handler and Specta, then run `just gen-types`.

- [ ] **Step 5: Verify GREEN**
      Run: `cargo test -p agent-runtime send_message_strict_rejects_running_session -- --exact`
      Expected: one test passes.

### Task 3: Cancellation Force Abort

**Files:**

- Modify: `crates/agent-runtime/src/execution_runtime/session_actor.rs`
- Test: `crates/agent-runtime/src/execution_runtime/session_actor_tests.rs`

- [ ] **Step 1: Write failing actor test**
      Add a stubborn executor that ignores cancellation forever. Cancel the actor and assert it returns to `Idle` within a bounded timeout.

- [ ] **Step 2: Verify RED**
      Run: `cargo test -p agent-runtime cancel_force_aborts_stubborn_turn -- --exact`
      Expected: timeout/failure because cancel only signals the token.

- [ ] **Step 3: Implement grace timeout**
      After `handle_cancel`, start a short internal force-abort timer for the active turn. If the join has not finished after the grace period, abort it and finish the running turn with a cancellation error.

- [ ] **Step 4: Verify GREEN**
      Run: `cargo test -p agent-runtime cancel_force_aborts_stubborn_turn -- --exact`
      Expected: one test passes.

### Task 4: Model Stream Retry Events

**Files:**

- Modify: `crates/agent-core/src/events.rs` or event payload definition location
- Modify: `crates/agent-runtime/src/agent_loop/stream_handler.rs`
- Test: `crates/agent-runtime/src/agent_loop/stream_handler_tests.rs`

- [ ] **Step 1: Write failing test**
      Extend the stream-start retry test to assert a session event is emitted for retrying and final timeout/stall metadata.

- [ ] **Step 2: Verify RED**
      Run: `cargo test -p agent-runtime stream_start_retry_emits_event -- --exact`
      Expected: fail because no event exists.

- [ ] **Step 3: Implement event**
      Add an event payload such as `ModelStreamStatus { phase, retrying, retry_attempt, max_retries, message }` and emit it when stream start retry/final timeout is logged.

- [ ] **Step 4: Verify GREEN**
      Run: `cargo test -p agent-runtime stream_start_retry_emits_event -- --exact`
      Expected: one test passes.

### Task 5: Full Verification and PR

**Files:**

- All modified files above.

- [ ] **Step 1: Format and focused tests**
      Run: `cargo fmt --all --check`
      Run focused tests added above.

- [ ] **Step 2: Crate checks**
      Run: `cargo test -p agent-runtime`
      Run: `cargo test -p agent-gui-tauri`
      Run: `cargo clippy -p agent-runtime --all-targets -- -D warnings`
      Run: `cargo clippy -p agent-gui-tauri --all-targets -- -D warnings`

- [ ] **Step 3: Generated bindings**
      Run: `just gen-types`
      Confirm generated diff only reflects the new command/event types.

- [ ] **Step 4: Dev App validation**
      Start `bun --filter agent-gui tauri dev --features pilot`, verify `tauri-pilot ping`, invoke `send_message_to_session` against a known busy session or inspect command availability, then clean port 1420.

- [ ] **Step 5: Commit, PR, watcher, merge**
      Commit intentionally, push, create PR to `main`, enable auto-merge, run PR watcher until merged, fast-forward local main, and clean the worktree/branch.

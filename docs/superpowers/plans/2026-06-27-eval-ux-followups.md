# Eval UX Followups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the highest-friction issues seen during the latest Kairox self-evaluation without building a new reporting system.

**Architecture:** Keep the PR narrow: make idle session sends return the accepted `UserMessageAdded.message_id`, stop new sessions from being titled with their initial model profile, and make `scripts/dev-pilot.sh` distinguish active compilation from real pilot failure while surfacing StarPoint recovery hints. Skip Run Summary and provider token usage in this PR because they cross GUI/runtime/model contracts and deserve their own scoped design.

**Tech Stack:** Rust Tauri commands/runtime tests, Bash wrapper tests/manual shell checks.

---

### Task 1: Idle Send Acknowledgement

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands/chat.rs`

- [x] **Step 1: Write failing assertions**

Update `send_message_to_session_if_idle_inner_runs_idle_turn` to assert the response includes `accepted_message_id` matching the persisted `UserMessageAdded.message_id`. Update duplicate/in-flight tests to assert duplicate accepted responses keep that message id and in-flight responses have none.

- [x] **Step 2: Run RED**

Run: `cargo test -p agent-gui-tauri send_message_to_session_if_idle --lib`
Expected: compile/test failure because `accepted_message_id` does not exist.

- [x] **Step 3: Implement minimal response field**

Add `accepted_message_id: Option<String>` to `SendMessageToSessionIfIdleResponse`. After `runtime.send_message_if_idle(request).await` succeeds, load the session trace and pick the latest matching `UserMessageAdded.message_id` for the sent display/model content. Store that id with the accepted client request state so duplicate retries return the same id.

- [x] **Step 4: Run GREEN**

Run: `cargo test -p agent-gui-tauri send_message_to_session_if_idle --lib`
Expected: pass.

### Task 2: Neutral Initial Session Titles

**Files:**

- Modify: `crates/agent-runtime/src/session.rs`
- Modify: `crates/agent-runtime/src/facade_sessions_tests.rs`
- Modify as needed: `crates/agent-runtime/src/session_tests.rs`, `crates/agent-store/src/event_store/tests/session_lifecycle.rs`

- [x] **Step 1: Write failing assertions**

Change session metadata expectations from `Session using fake` / `Session using fast` to `New conversation`.

- [x] **Step 2: Run RED**

Run: `cargo test -p agent-runtime start_session_persists_metadata`
Expected: failure while production still writes `Session using <profile>`.

- [x] **Step 3: Implement minimal title change**

Change new session metadata title to `New conversation`. Leave explicit renames and first-message projection behavior untouched.

- [x] **Step 4: Run GREEN**

Run: `cargo test -p agent-runtime start_session_persists_metadata`
Expected: pass.

### Task 3: Dev Pilot Startup Diagnostics

**Files:**

- Modify: `scripts/dev-pilot.sh`

- [x] **Step 1: Add shell helpers**

Add startup activity and StarPoint hint helpers. Startup detection should return true when a child command is still alive and logs are recently updated or show cargo/Tauri startup output; `_print_starpoint_hint` should print the exact `STARPOINT_EXPECT="$REPO_ROOT/target/debug/agent-gui-tauri"` helper command when logs or exit status show `Killed: 9`, `SIGKILL`, or `signal 9`.

- [x] **Step 2: Wire helpers into pilot waits**

When pilot is not ready but build is active, print `Still waiting: <label> is compiling or starting...` and continue waiting. When a command exits before pilot readiness, tail logs and print the StarPoint hint if matched.

- [x] **Step 3: Avoid stale dev targets**

Make the wrapper's port probe match Vite's `0.0.0.0` bind behavior and remove a pre-existing target pilot socket before launching, so an already-open GUI does not cause a stale socket false positive.

- [x] **Step 4: Shell validation**

Run: `bash -n scripts/dev-pilot.sh`
Expected: pass.

### Task 4: Final Gates

- [x] Run `cargo fmt --all`.
- [x] Run `cargo fmt --all --check`.
- [x] Run `cargo test -p agent-gui-tauri send_message_to_session_if_idle --lib`.
- [x] Run `cargo test -p agent-runtime start_session_persists_metadata`.
- [x] Run `cargo test -p agent-store upsert_and_list_active_sessions`.
- [x] Run `cargo check --workspace --all-targets`.
- [x] Run `bash -n scripts/dev-pilot.sh`.
- [x] Dev App validation: start `scripts/dev-pilot.sh`, confirm `tauri-pilot ping`, inspect `tauri-pilot snapshot -i`, check `tauri-pilot logs --level error`, and verify `list_sessions` returns `New conversation`.

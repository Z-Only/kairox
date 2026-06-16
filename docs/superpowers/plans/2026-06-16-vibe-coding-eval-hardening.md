# Vibe Coding Eval Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Harden Kairox project-session/vibe-coding automation based on the risk-command-const-arrays evaluation: completed model streams must terminate cleanly, automation must have an explicit waitable send path, shell command exit codes must be structured, post-tool stream stalls must be diagnosable, and eval/SKILL gates must catch format and CR-loop regressions.

**Architecture:** Keep the existing per-session FIFO actor queue for follow-up turns. Add narrowly scoped runtime metadata and GUI IPC surfaces instead of changing chat UX. Preserve existing event schemas where possible; use existing `ToolInvocationCompleted.exit_code` by carrying exit status through `ToolOutput`. Update eval fixture and ignored local SKILL files so the behavior is exercised by both automated regression and the Kairox-on-Kairox workflow.

**Tech Stack:** Rust (`agent-runtime`, `agent-tools`, `agent-core`, `agent-gui-tauri`, `agent-eval`), Tauri/Specta generated TypeScript, Kairox local SKILL markdown.

---

## File Structure

- Modify `crates/agent-runtime/src/agent_loop/stream_handler.rs`
  - Treat `ModelEvent::Completed` as a terminal stream event.
  - Enrich stream timeout status with request stats that automation can consume.
- Modify `crates/agent-runtime/src/agent_loop/stream_handler_tests.rs`
  - Add RED/GREEN tests for completed streams that never close and for post-tool timeout event diagnostics.
- Modify `crates/agent-tools/src/registry.rs`
  - Add optional structured `exit_code` to `ToolOutput`.
- Modify `crates/agent-tools/src/shell/exec.rs`
  - Populate `ToolOutput.exit_code` for shell success and failure.
- Modify `crates/agent-tools/src/shell/tests.rs`
  - Add RED/GREEN tests for shell success/failure exit codes.
- Modify `crates/agent-runtime/src/agent_loop/tool_loop.rs`
  - Copy `ToolOutput.exit_code` into `EventPayload::ToolInvocationCompleted.exit_code`.
- Modify `crates/agent-runtime/tests/agent_loop/tool_calls.rs`
  - Add integration coverage that `shell.exec` completion events include exit codes.
- Modify `crates/agent-runtime/src/facade_session_ops.rs`
  - Add explicit queue-named APIs while preserving current follow-up queue semantics.
- Modify `apps/agent-gui/src-tauri/src/commands/chat.rs`
  - Add a waitable project/session send command for automation.
- Modify `apps/agent-gui/src-tauri/src/lib.rs`
  - Register the new command.
- Modify `apps/agent-gui/src-tauri/src/specta.rs`
  - Export the new command.
- Regenerate `apps/agent-gui/src/generated/commands.ts`
  - Reflect the new Tauri command.
- Modify `apps/agent-gui/src-tauri/src/commands/chat_tests.rs` or existing command tests if present.
  - Compile-time command coverage for the new IPC function.
- Modify `crates/agent-eval/fixtures/live-vibe-coding.jsonl`
  - Add `cargo fmt --all --check` to the vibe-coding risk-command post-run commands.
- Modify ignored local files outside this worktree:
  - `/Users/chanyu/AIProjects/kairox/.agents/skills/kairox-dev-workflow/SKILL.md`
  - `/Users/chanyu/AIProjects/kairox/.agents/skills/kairox-evaluate-kairox/SKILL.md`

Forbidden:

- Do not remove same-session follow-up queueing; existing project chat continuations rely on it.
- Do not change public event variant names.
- Do not edit generated TypeScript by hand; run `just gen-types`.
- Do not add root-crate re-export requirements to SKILL docs in this task; user requested only SKILL/Eval items 3, 5, and 6.

## Task 1: Completed Model Streams Are Terminal

**Files:**

- Modify: `crates/agent-runtime/src/agent_loop/stream_handler_tests.rs`
- Modify: `crates/agent-runtime/src/agent_loop/stream_handler.rs`

- [x] **Step 1: Write failing test**
      Add a test using a fake model stream that emits `ModelEvent::TokenDelta("done")` then `ModelEvent::Completed { usage: None }`, but never returns `None`. Call `process_model_stream_with_idle_timeout` with a short timeout and assert it returns `assistant_text == "done"` rather than timing out.

- [x] **Step 2: Verify RED**
      Run:
      `cargo test -p agent-runtime completed_model_event_ends_stream_without_waiting_for_eof -- --exact`
      Expected: FAIL with a timeout because `Completed` currently does not break the stream loop.

- [x] **Step 3: Implement minimal code**
      In `process_model_stream_with_idle_timeout`, after handling `ModelEvent::Completed`, break the event loop once content or tool calls have been produced. Preserve Anthropic `message_start` usage-only completions (`output_tokens == 0` before content) as progress events and continue reading the stream. Keep the existing `AssistantMessageCompleted` emission when `assistant_text` is non-empty.

- [x] **Step 4: Verify GREEN**
      Run:
      `cargo test -p agent-runtime completed_model_event_ends_stream_without_waiting_for_eof -- --exact`
      Expected: PASS with one non-zero test.

## Task 2: Structured Shell Exit Codes

**Files:**

- Modify: `crates/agent-tools/src/registry.rs`
- Modify: `crates/agent-tools/src/shell/exec.rs`
- Modify: `crates/agent-tools/src/shell/tests.rs`
- Modify: `crates/agent-runtime/src/agent_loop/tool_loop.rs`
- Modify: `crates/agent-runtime/tests/agent_loop/tool_calls.rs`

- [x] **Step 1: Write failing shell tests**
      Add assertions:
  - `shell_exec_readonly_command_succeeds` expects `result.exit_code == Some(0)`.
  - `shell_exec_captures_stderr_on_failure` expects `result.exit_code == Some(nonzero)`.

- [x] **Step 2: Verify RED**
      Run:
      `cargo test -p agent-tools shell_exec_readonly_command_succeeds shell_exec_captures_stderr_on_failure -- --nocapture`
      Expected: FAIL to compile because `ToolOutput.exit_code` does not exist.

- [x] **Step 3: Implement ToolOutput exit_code**
      Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub exit_code: Option<i32>` to `ToolOutput`.
      Update all `ToolOutput` constructors in the repo to set `exit_code: None` unless they know a process exit status.
      In `shell.exec`, set `Some(0)` on success and `Some(exit_code)` on non-zero exit.

- [x] **Step 4: Verify shell GREEN**
      Run:
      `cargo test -p agent-tools shell_exec_readonly_command_succeeds -- --exact`
      `cargo test -p agent-tools shell_exec_captures_stderr_on_failure -- --exact`
      Expected: both pass.

- [x] **Step 5: Write failing runtime event test**
      Add or extend a runtime tool-call test so a `shell.exec` invocation produces `ToolInvocationCompleted { exit_code: Some(0), ... }`.

- [x] **Step 6: Verify RED**
      Run:
      `cargo test -p agent-runtime tool_invocation_completed_records_shell_exit_code -- --exact`
      Expected: FAIL because `tool_loop.rs` still writes `exit_code: None`.

- [x] **Step 7: Implement runtime propagation**
      Change `tool_loop.rs` completion event to use `exit_code: output.exit_code`.

- [x] **Step 8: Verify GREEN**
      Run:
      `cargo test -p agent-runtime tool_invocation_completed_records_shell_exit_code -- --exact`
      Expected: PASS.

## Task 3: Explicit Queued and Waitable Session Sends

**Files:**

- Modify: `crates/agent-runtime/src/facade_session_ops.rs`
- Modify: `crates/agent-runtime/src/facade_runtime/tests/send_message_tests.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands/chat.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Regenerate: `apps/agent-gui/src/generated/commands.ts`

- [x] **Step 1: Add runtime API tests**
      Add tests proving:
  - Existing strict/follow-up path still queues behind a running turn.
  - A new explicitly named `send_message_queued` or equivalent helper has the same queue behavior.
  - Cancelling and compacting still reject.

- [x] **Step 2: Verify RED**
      Run:
      `cargo test -p agent-runtime send_message_queued -- --nocapture`
      Expected: FAIL because the explicit queued API does not exist.

- [x] **Step 3: Add runtime API alias**
      Add a clearly named runtime method for queueing same-session follow-ups. Keep `send_message_strict` as a compatibility wrapper if needed, but route GUI automation through the queue-named method.

- [x] **Step 4: Add waitable Tauri command**
      Add `send_message_to_session_and_wait(session_id, content, attachments)` that:
  - prepares/enriches outbound content exactly like `send_message_to_session`,
  - sends through the queue-named runtime method,
  - awaits completion/errors before returning to IPC.
    The existing non-waiting command stays unchanged for chat UI responsiveness.

- [x] **Step 5: Add compile-time command coverage**
      Add a focused Tauri command compile test referencing `send_message_to_session_and_wait`.

- [x] **Step 6: Verify GREEN and generate bindings**
      Run:
      `cargo test -p agent-runtime send_message_queued -- --nocapture`
      `cargo test -p agent-gui-tauri send_message_to_session_and_wait -- --nocapture`
      `just gen-types`
      Expected: tests pass and generated command bindings include the new command.

## Task 4: Post-Tool Stream Stall Diagnostics

**Files:**

- Modify: `crates/agent-runtime/src/agent_loop/stream_handler.rs`
- Modify: `crates/agent-runtime/src/agent_loop/stream_handler_tests.rs`

- [x] **Step 1: Write failing diagnostic test**
      Add a stream timeout test whose `ModelRequest` contains tool results and assistant tool-call messages. Assert the emitted `ModelStreamStatus.message` contains `tool_results=<n>` and `assistant_tool_messages=<n>` so automation can diagnose post-tool stalls from events, not logs.

- [x] **Step 2: Verify RED**
      Run:
      `cargo test -p agent-runtime stream_event_timeout_status_includes_request_stats -- --exact`
      Expected: FAIL because event message currently contains only the generic timeout text.

- [x] **Step 3: Implement enriched event message**
      For timeout status events, emit the same contextual string produced by `model_stream_timeout_error_with_context` instead of the generic log label. Keep log labels unchanged for readability.

- [x] **Step 4: Verify GREEN**
      Run:
      `cargo test -p agent-runtime stream_event_timeout_status_includes_request_stats -- --exact`
      Expected: PASS.

## Task 5: Eval Fixture Format Gate

**Files:**

- Modify: `crates/agent-eval/fixtures/live-vibe-coding.jsonl`
- Modify: `crates/agent-eval/tests/cli.rs` if fixture assertions need coverage.

- [x] **Step 1: Add failing fixture assertion test**
      Extend CLI fixture tests to assert the risk-command scenario contains a post-run `cargo fmt --all --check` command.

- [x] **Step 2: Verify RED**
      Run:
      `cargo test -p agent-eval live_vibe_coding_risk_scenario_has_format_gate -- --exact`
      Expected: FAIL.

- [x] **Step 3: Add post-run format command**
      Insert the `cargo fmt --all --check` post-run command before focused tests in `live-vibe-coding.jsonl`.

- [x] **Step 4: Verify GREEN**
      Run:
      `cargo test -p agent-eval live_vibe_coding_risk_scenario_has_format_gate -- --exact`
      Expected: PASS.

## Task 6: SKILL Format and CR Loop Hardening

**Files outside tracked worktree:**

- Modify: `/Users/chanyu/AIProjects/kairox/.agents/skills/kairox-dev-workflow/SKILL.md`
- Modify: `/Users/chanyu/AIProjects/kairox/.agents/skills/kairox-evaluate-kairox/SKILL.md`

- [x] **Step 1: Update format gate wording**
      In `kairox-dev-workflow`, require `cargo fmt --all` then `cargo fmt --all --check` before tests/lint for code tasks, and require final report evidence for format.

- [x] **Step 2: Update CR continuation wording**
      In `kairox-evaluate-kairox`, split continuation budgets into runtime-stuck continuations and CR-fix continuations. Clarify that format-gate failures count as CR-fix turns.

- [x] **Step 3: Manual verification**
      Run:
      `rg "cargo fmt --all|CR-fix|format" /Users/chanyu/AIProjects/kairox/.agents/skills/kairox-dev-workflow/SKILL.md /Users/chanyu/AIProjects/kairox/.agents/skills/kairox-evaluate-kairox/SKILL.md`
      Expected: the new requirements are visible.

## Task 7: Full Verification

**Files:**

- All modified tracked and ignored files.

- [x] **Step 1: Focused tests**
      Run every focused test named above and confirm non-zero test counts.

- [x] **Step 2: Crate gates**
      Run:
      `cargo fmt --all --check`
      `cargo clippy -p agent-runtime --all-targets -- -D warnings`
      `cargo clippy -p agent-tools --all-targets -- -D warnings`
      `cargo clippy -p agent-gui-tauri --all-targets -- -D warnings`
      `cargo test -p agent-runtime`
      `cargo test -p agent-tools`
      `cargo test -p agent-gui-tauri`
      `cargo test -p agent-eval`

- [x] **Step 3: GUI / Dev App validation**
      Run `bun --filter agent-gui tauri dev --features pilot`, connect with `tauri-pilot ping`, and invoke or inspect availability of `send_message_to_session_and_wait`. Stop port 1420 afterwards.

- [x] **Step 4: Final status**
      Report tracked worktree diff, ignored SKILL diff summary, verification results, and any blocker.

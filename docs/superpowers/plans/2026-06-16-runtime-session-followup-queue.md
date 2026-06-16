# Runtime Session Follow-up Queue

## Goal

Allow GUI/project follow-up messages sent to an already-running session to enter the existing per-session FIFO turn queue instead of being rejected as `SessionBusy`, while preserving the busy rejection for compaction and cancellation.

## Context

The session execution actor already queues same-session `RunTurn` commands. The observed failure came from `send_message_to_session` using the strict preflight and strict send path, where `ensure_session_can_send(..., reject_active_execution = true)` rejects `ExecutionState::Running`.

## Tasks

1. Update the runtime send-message test that currently expects strict send to reject a running session.
   - Expected RED: strict send should queue behind the first turn and only finish after the first turn releases.
   - Keep existing compaction busy coverage unchanged.
2. Change the runtime guard so active `Running` turns are accepted for follow-up queuing, while `Cancelling` remains busy.
   - Keep project-session visibility marking before the actor receives the queued turn.
   - Keep compaction busy checks before queuing.
3. Verify focused runtime behavior.
   - Run the focused failing test first.
   - Run `cargo test -p agent-runtime send_message`.
   - Run `cargo clippy -p agent-runtime --all-targets -- -D warnings`.
   - Run `cargo fmt --all --check`.

## Dev App

This is a Rust runtime behavior change that affects the GUI command path, but the core regression is deterministic at runtime level. If full Dev App/pilot is too costly for this narrow queue behavior, document focused Rust verification as the fallback.

## Verification Log

- `cargo test -p agent-runtime send_message_strict_queues_same_session_turn_when_actor_turn_running -- --nocapture` — PASS; 1 matching lib test passed.
- `cargo test -p agent-runtime send_message` — PASS; matching send-message tests passed, including 8 lib tests plus matching integration tests.
- `cargo clippy -p agent-runtime --all-targets -- -D warnings` — PASS.
- `cargo fmt --all --check` — PASS.
- `cargo test -p agent-runtime` — PASS; 686 lib tests and all agent-runtime integration tests passed.
- Dev App/pilot not run for this lane; used focused Rust runtime fallback because the changed behavior is deterministic in `agent-runtime` and covered by actor/facade tests.

# Tool Temp Artifacts and Stream Observability

## Scope

Fix two issues found while evaluating Kairox-on-Kairox execution:

- `fs.write` should not leave persistent `.bak` files by default.
- model stream start idle-timeout logs should make recoverable retries visibly distinct from final failures.

SKILL documentation fixes for the same evaluation are handled directly in the main checkout because `.agents/skills/**` is ignored by git.

## Touched Files

- `crates/agent-tools/src/fs_write.rs`
- `crates/agent-tools/src/fs_write_tests.rs`
- `crates/agent-runtime/src/agent_loop/stream_handler.rs`
- `crates/agent-runtime/src/agent_loop/stream_handler_tests.rs`

## Forbidden Files

- GUI generated bindings.
- Event schema and stored event payloads.
- Model provider implementations outside the stream handler.
- Permission policy semantics.

## Acceptance Signals

- A default overwrite through `fs.write` updates the target file and leaves no `<file>.bak`.
- Explicit `backup: true` keeps the old backup behavior.
- `fs.write` tool schema documents the new opt-in backup flag.
- Recoverable stream-start idle timeouts log as retrying and include retry metadata.
- Final stream idle timeouts still log as a terminal timeout/failure path.

## TDD Plan

1. RED: add `fs.write` tests proving default overwrite leaves no `.bak`, and backup only happens when requested.
2. GREEN: add optional `backup` boolean argument defaulting to `false`; preserve atomic temp-file write.
3. RED: add stream handler tests for retry timeout log classification helper.
4. GREEN: mark retry timeout progress with retry metadata and use a distinct log message/fields for retrying vs final timeout.

## Verification

- `cargo test -p agent-tools fs_write`
- `cargo test -p agent-runtime stream_start_timeout_retries_before_failing_turn`
- `cargo test -p agent-runtime model_stream_timeout_log_classification`
- `cargo fmt --all --check`
- Pre-push gates from `.agents/skills/kairox-dev-workflow/references/pre-push-gates.md` as time/runtime permits.

## Dev App Verification

These are backend tool/runtime behavior changes. If full Dev App verification is feasible before PR, run `bun --filter agent-gui tauri dev --features pilot` and verify a local pilot session can execute a simple message without new backend errors. If runtime is blocked, record the exact blocker and retain targeted Rust test evidence.

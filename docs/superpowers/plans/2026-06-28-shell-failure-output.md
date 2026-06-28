# Shell Failure Output Plan

## Goal

Make `shell.exec` failures observable when a command exits non-zero and writes diagnostics to stdout instead of stderr.

## Scope

Owned files:

- `crates/agent-tools/src/shell/exec.rs`
- `crates/agent-tools/src/shell/tests.rs`

Forbidden files:

- GUI chat stream/session/composer code
- Dev App startup scripts
- dependency/bootstrap scripts
- generated bindings

## TDD

RED:

- Add a focused shell tool test where a command prints to stdout and exits with a non-zero status.
- Run the new test and confirm current behavior omits the stdout diagnostic.

GREEN:

- Update failure formatting to use stdout when stderr is empty or whitespace-only.
- Preserve existing stderr-first behavior for failing commands that write stderr.
- Run the new test and existing shell failure tests.

## Quality Gates

- `cargo fmt --all`
- `cargo fmt --all --check`
- `cargo test -p agent-tools shell_exec`
- `cargo clippy -p agent-tools --all-targets -- -D warnings`

## Dev App Validation

Not planned for this PR. The change is inside `agent-tools` output formatting with no GUI, Tauri IPC, store, or session behavior changes; unit tests cover the changed behavior directly.

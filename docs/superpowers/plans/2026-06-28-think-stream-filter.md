# Think Stream Filter Plan

## Goal

Prevent raw `<think>...</think>` reasoning blocks from being emitted as streaming `ModelTokenDelta` events.

## Scope

Owned files:

- `crates/agent-runtime/src/agent_loop/stream_handler.rs`
- `crates/agent-runtime/src/agent_loop/stream_handler_tests.rs`

Forbidden files:

- GUI chat components and stores
- TUI renderer
- model provider parsers
- generated bindings

## TDD

RED:

- Add a stream handler test where a think block is split across token deltas.
- Assert stored `ModelTokenDelta` events do not contain raw think tag/content and only emit visible text.

GREEN:

- Keep accumulating raw assistant text for final completion logic.
- Emit only the newly visible suffix after applying streaming-safe think filtering.
- Preserve existing completed-message filtering and the unclosed literal `<think>` negative case.

## Quality Gates

- `cargo fmt --all`
- `cargo fmt --all --check`
- `cargo test -p agent-runtime stream_`
- `cargo clippy -p agent-runtime --all-targets -- -D warnings`

## Dev App Validation

Not planned for this PR. The changed contract is deterministic runtime event filtering and is covered by focused `agent-runtime` tests; no GUI rendering, Tauri IPC, store schema, or generated bindings change.

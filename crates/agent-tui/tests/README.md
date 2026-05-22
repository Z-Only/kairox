# agent-tui test architecture

The TUI tests are layered by how much of the terminal stack they exercise.

| Layer | Scope                                                                   | Tooling                                                                    | Primary files                                                              |
| ----- | ----------------------------------------------------------------------- | -------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| L0    | Pure state, keybinding, reducer, and component behavior                 | ordinary `#[test]` / `#[tokio::test]` in `src/**`                          | `src/app/**`, `src/components/**`, `src/keybindings.rs`                    |
| L1    | Deterministic rendering of widgets and composed app regions             | Ratatui `TestBackend`; `insta` for stable snapshots                        | `snapshot_tests.rs`, `full_app_render.rs`                                  |
| L2    | Keyboard flows through `App::handle_crossterm_event` without a real TTY | lightweight harness + Ratatui `TestBackend`                                | `parity_terminal_harness.rs`, `parity_smoke.rs`                            |
| L3    | Runtime/facade integration without terminal bytes                       | `LocalRuntime`, fake model, in-memory SQLite                               | `app_logic.rs`, `interactive_session.rs`, `command_dispatch_boundaries.rs` |
| L4    | Terminal integration that `TestBackend` cannot observe                  | `portable-pty` real PTY smoke, ignored by default and run explicitly in CI | `terminal_pty_smoke.rs`                                                    |

Use the lowest layer that can fail for the behavior being changed. Keep L4 small:
it validates raw-mode input, alternate-screen startup, and representative overlay
paths, while most UI behavior remains in deterministic L0-L3 tests.

Useful commands:

```bash
just test-tui
just test-tui-pty
cargo test -p agent-tui
cargo test -p agent-tui snapshot_tests
cargo test -p agent-tui parity_smoke
cargo test -p agent-tui --test terminal_pty_smoke -- --ignored --nocapture
```

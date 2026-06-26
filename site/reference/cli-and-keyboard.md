---
title: CLI & Keyboard
description: Every `just` recipe, every `bun` script, the TUI keymap, and the GUI keyboard shortcuts.
outline: [2, 3]
---

# CLI & Keyboard

Kairox is a workspace, a desktop app, and a terminal app. That means three kinds of muscle memory: shell commands that run the workspace, keystrokes that drive the TUI, and keystrokes that drive the GUI. This page is the lookup table.

## `just` recipes

Kairox uses [`just`](https://github.com/casey/just) as the task runner. Install with `cargo install just`; list everything with `just --list`.

### Quick checks

| Recipe           | What it does                                                                     |
| ---------------- | -------------------------------------------------------------------------------- |
| `just fmt-check` | Run all formatters in check mode (`cargo fmt --check` plus `oxfmt --check`).     |
| `just lint`      | Run Clippy across the workspace, plus `oxlint` and Stylelint on the GUI sources. |
| `just test`      | `cargo test --workspace --all-targets`.                                          |
| `just test-gui`  | GUI Vitest suite plus GUI script tests.                                          |
| `just coverage`  | Rust source-based coverage gate plus GUI V8 coverage gate.                       |
| `just check`     | Format-check + lint + Rust tests. The full local CI gate.                        |

### Formatting

| Recipe     | What it does                                              |
| ---------- | --------------------------------------------------------- |
| `just fmt` | Auto-format Rust (`cargo fmt`) and web sources (`oxfmt`). |

### Development

| Recipe                  | What it does                                                                              |
| ----------------------- | ----------------------------------------------------------------------------------------- |
| `just tui`              | Run the TUI app (`cargo run -p agent-tui`).                                               |
| `just gui-dev`          | Run the GUI dev server (Vite hot-reload). Regenerates TS types first.                     |
| `just tauri-dev`        | Run the Tauri desktop app in dev mode (Vite + native window). Regenerates TS types first. |
| `just gui-build`        | Build GUI web assets.                                                                     |
| `just tauri-build`      | Build the Tauri desktop binary plus platform installers.                                  |
| `just tauri-build-fast` | Build the Tauri desktop binary without bundling installers (faster local iteration).      |
| `just gui-size`         | Build the GUI and print the largest generated files.                                      |
| `just rust-size`        | Print release-binary sizes for `agent-tui` and `agent-gui-tauri` (must be built first).   |

### Release

| Recipe                        | What it does                                                                                                               |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `just release <version> ...`  | Run `scripts/release.sh` for the named version.                                                                            |
| `just release-dry <version>`  | Preview what `release.sh` would do without executing anything.                                                             |
| `just changelog <tag>`        | Run `git cliff --tag <tag>` and format the output.                                                                         |
| `just bump-version <version>` | Sync version across `Cargo.toml`, `Cargo.lock`, root `package.json`, `apps/agent-gui/package.json`, and `tauri.conf.json`. |

### Worktree

| Recipe                 | What it does                                                                                                          |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `just worktree <name>` | Create a sibling git worktree under `.worktrees/<sanitized-name>` branched from `main`, then `bun install` inside it. |

### Type sync and codegen

| Recipe             | What it does                                                                                                      |
| ------------------ | ----------------------------------------------------------------------------------------------------------------- |
| `just gen-types`   | Regenerate `apps/agent-gui/src/generated/{commands,events}.ts` from Tauri commands and `EventPayload` via Specta. |
| `just check-types` | Run `just gen-types` and fail if the generated files differ from what is checked in.                              |

### Integration and end-to-end tests

| Recipe                 | What it does                                                                                                                                      |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `just test-e2e`        | Playwright E2E tests for the GUI frontend (against the Tauri IPC mock).                                                                           |
| `just test-e2e-headed` | Same as `test-e2e` in headed (visible browser) mode for debugging.                                                                                |
| `just test-e2e-ui`     | Same as `test-e2e` in the Playwright UI runner.                                                                                                   |
| `just test-tui`        | Deterministic TUI test layers — no real terminal required.                                                                                        |
| `just test-tui-pty`    | Real-PTY TUI smoke test (the one CI runs). Builds the binary first.                                                                               |
| `just test-fullstack`  | Full-stack runtime integration tests.                                                                                                             |
| `just test-all`        | `test` + `test-tui` + `test-fullstack` + `test-gui`.                                                                                              |
| `just test-mcp`        | All MCP-related tests across `agent-mcp`, `agent-tools`, `agent-config`, and `agent-runtime`.                                                     |
| `just test-live`       | GitHub Models live smoke test (self-skips without `GITHUB_TOKEN`).                                                                                |
| `just test-pilot`      | Start the Tauri dev app with the `pilot` feature and run the `tauri-pilot` E2E scenarios. Requires `tauri-pilot-cli`; use `xvfb-run -a` on Linux. |
| `just test-pilot-live` | `test-pilot` with `KAIROX_PILOT_LIVE_MODELS=1` — runs against real GitHub Models. Requires `GITHUB_TOKEN`.                                        |

## `bun` scripts

The root `package.json` exposes Bun-runnable scripts. Most of them are wrapped by `just`; reach for them directly when you only want one stage.

| Script                       | What it does                                                                                                     |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `bun run format`             | Run all formatters in write mode.                                                                                |
| `bun run format:check`       | Run all formatters in check mode (no writes).                                                                    |
| `bun run format:rust`        | Just `cargo fmt --all`.                                                                                          |
| `bun run format:web`         | Just `oxfmt --write .`.                                                                                          |
| `bun run lint`               | Web lint + Rust lint + the TUI/GUI parity-matrix check.                                                          |
| `bun run lint:web`           | `oxlint` plus Stylelint.                                                                                         |
| `bun run lint:rust`          | Clippy with `-D warnings` across the workspace.                                                                  |
| `bun run lint:parity-matrix` | Custom script that ensures TUI/GUI feature parity is tracked.                                                    |
| `bun run lint:style`         | Stylelint only.                                                                                                  |
| `bun run lint:oxlint`        | oxlint only.                                                                                                     |
| `bun run site:dev`           | VitePress dev server for the documentation site.                                                                 |
| `bun run site:build`         | Build the VitePress site and run `scripts/generate-llms-txt.mjs` to emit `dist/llms.txt` + `dist/llms-full.txt`. |
| `bun run site:preview`       | Preview the built documentation site.                                                                            |
| `bun run coverage:rust`      | Rust source-based coverage gate (`scripts/run-rust-coverage.sh`).                                                |
| `bun run coverage:web`       | GUI V8 coverage gate (Vitest).                                                                                   |
| `bun run prepare`            | Husky install hook. Runs automatically after `bun install`.                                                      |

Inside the GUI app (`apps/agent-gui/package.json`), these scripts exist:

| Script (run inside `apps/agent-gui` or via `bun --filter agent-gui`) | What it does                                     |
| -------------------------------------------------------------------- | ------------------------------------------------ |
| `dev`                                                                | Vite dev server, defaulting to `0.0.0.0:1420`.   |
| `build`                                                              | Vite production build.                           |
| `tauri:dev`                                                          | Tauri dev (Vite + native window).                |
| `tauri:build`                                                        | Tauri production build (with installer bundles). |
| `test`                                                               | Vitest unit suite.                               |
| `test:e2e`                                                           | Playwright E2E with 2 workers.                   |
| `test:e2e:headed` / `test:e2e:ui`                                    | Headed / UI variants of the Playwright suite.    |

## TUI keymap

The TUI is built on `ratatui` + `crossterm`. The resolver in `crates/agent-tui/src/keybindings/resolver.rs` is the source of truth.

### Global

| Key          | Action                                                                     |
| ------------ | -------------------------------------------------------------------------- |
| `F1`         | Open the help overlay.                                                     |
| `Tab`        | Cycle focus between panels (Chat → Sessions → Trace).                      |
| `Esc`        | Escape the current overlay / cancel the current modal / leave search mode. |
| `Ctrl+C`     | Interrupt the active turn. With no turn in flight, quits the app.          |
| `Ctrl+Enter` | Send the composed input regardless of focus or input mode.                 |
| `Ctrl+P`     | Toggle the command palette.                                                |

### Alt-modifier toggles (overlays, sidebars, focus)

| Key     | Action                                        |
| ------- | --------------------------------------------- |
| `Alt+1` | Focus the Chat panel.                         |
| `Alt+2` | Focus the Sessions sidebar.                   |
| `Alt+3` | Focus the Trace sidebar.                      |
| `Alt+S` | Toggle the Sessions sidebar.                  |
| `Alt+T` | Toggle the Trace sidebar.                     |
| `Alt+E` | Toggle input mode (single-line ↔ multi-line). |
| `Alt+P` | Open the profile selector.                    |
| `Alt+C` | Toggle context-details panel.                 |
| `Alt+N` | Start a new session.                          |
| `Alt+Q` | Quit.                                         |
| `Alt+H` | Toggle the Hooks overlay.                     |
| `Alt+I` | Toggle the Instructions overlay.              |

### Ctrl-modifier overlays

| Key      | Action                                             |
| -------- | -------------------------------------------------- |
| `Ctrl+G` | Toggle the Plugins overlay.                        |
| `Ctrl+L` | Toggle the Model overlay (active model + budgets). |
| `Ctrl+M` | Toggle the MCP overlay (server status).            |
| `Ctrl+S` | Toggle the Skills overlay.                         |

### Chat panel commands

The TUI chat input supports colon-prefixed commands:

| Command              | Action                    |
| -------------------- | ------------------------- |
| `:monitors`          | List active monitors.     |
| `:monitor stop <id>` | Stop a monitor by its ID. |

### Chat panel (focused)

| Key                      | Action                                                  |
| ------------------------ | ------------------------------------------------------- |
| `Enter`                  | In single-line mode: send. In multi-line mode: newline. |
| `Ctrl+Enter`             | Send the composed input regardless of mode.             |
| `Up` / `Down`            | Cycle input history.                                    |
| `Alt+Up`/`Down`          | Select prev/next queued message.                        |
| `Alt+Left`/`Right`       | Move the selected queued message up/down in the queue.  |
| `Alt+Enter`              | Send the selected queued message immediately.           |
| `Alt+Delete`/`Backspace` | Delete the selected queued message.                     |
| `Backspace`              | Erase one character.                                    |
| `Delete`                 | Forward delete.                                         |

### Sessions panel (focused)

| Key     | Action                          |
| ------- | ------------------------------- |
| `Enter` | Select the highlighted session. |
| `F2`    | Rename the highlighted session. |
| `A`     | Open the Archive manager.       |

### Trace panel (focused)

| Key              | Action                                           |
| ---------------- | ------------------------------------------------ |
| `Left` / `Right` | Cycle trace tabs (or `[` / `]`).                 |
| `F5`             | Toggle trace density (compact ↔ detailed).       |
| `/`              | Start memory search.                             |
| `S`              | Cycle memory scope (session / user / workspace). |
| `R`              | Retry the selected task.                         |
| `C`              | Cancel the selected task.                        |
| `Y`              | Confirm memory deletion.                         |
| `D`              | Delete the selected memory.                      |

### Permission prompt

When a permission modal is showing:

| Key   | Action                                             |
| ----- | -------------------------------------------------- |
| `Y`   | Approve this call.                                 |
| `N`   | Deny this call.                                    |
| `D`   | Deny this call **and** all future identical calls. |
| `Esc` | Deny (same as `N`).                                |

### Policy cycling

| Key             | Action                                               |
| --------------- | ---------------------------------------------------- |
| `A` (uppercase) | Cycle the active session's permission policy (mode). |
| `B` (uppercase) | Cycle the sandbox policy.                            |
| `x`             | Open the context menu for the focused item.          |

## GUI keyboard shortcuts

The desktop app inherits the standard OS shortcuts for the application chrome (Tauri provides Cmd+Q / Alt+F4, window cycling, etc.). The Kairox-specific shortcuts:

### Chat

| Key               | Action                                                |
| ----------------- | ----------------------------------------------------- |
| `Enter`           | Send the message (composer focused, no modifier).     |
| `Shift+Enter`     | Insert newline in the composer.                       |
| `j` / `ArrowDown` | Move focus to the next stream item in the chat panel. |
| `k` / `ArrowUp`   | Move focus to the previous stream item.               |
| `/`               | Trigger the inline command palette (composer empty).  |
| `@`               | Trigger the file-mention palette (composer).          |

### Command and mention palettes

| Key                     | Action                        |
| ----------------------- | ----------------------------- |
| `ArrowDown` / `ArrowUp` | Move highlight.               |
| `Enter`                 | Select the highlighted entry. |
| `Esc`                   | Close the palette.            |

### Editable labels (session names, etc.)

| Key     | Action            |
| ------- | ----------------- |
| `Enter` | Confirm the edit. |
| `Esc`   | Cancel the edit.  |

### Reserved modifier combos

The chat panel ignores keystrokes that include any modifier (Ctrl, Cmd, Alt) when not editing — these are reserved for host shortcuts the app may install in the future (Cmd+K palette, Ctrl+J line break, Alt+G workspace nav). If you find a missing shortcut, prefer adding it through a global handler rather than overloading the chat panel.

## Where the source of truth lives

- **Recipes**: [`justfile`](https://github.com/Z-Only/kairox/blob/main/justfile) at the repo root.
- **Scripts**: root [`package.json`](https://github.com/Z-Only/kairox/blob/main/package.json) and [`apps/agent-gui/package.json`](https://github.com/Z-Only/kairox/blob/main/apps/agent-gui/package.json).
- **TUI keymap**: [`crates/agent-tui/src/keybindings/resolver.rs`](https://github.com/Z-Only/kairox/blob/main/crates/agent-tui/src/keybindings/resolver.rs).
- **GUI keyboard handlers**: the `@keydown` bindings in [`apps/agent-gui/src/components/`](https://github.com/Z-Only/kairox/tree/main/apps/agent-gui/src/components).

If a binding in this page disagrees with the source, the source wins. File an issue so this page can be corrected.

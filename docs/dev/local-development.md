# Local Development

## Prerequisites

- Rust stable toolchain (pinned by `rust-toolchain.toml`)
- Node.js 22+
- Bun 1.3+
- [just](https://github.com/casey/just) task runner (`cargo install just` or `brew install just`)

## Quick start

```bash
bun install
just check      # format check + lint + test
just tui        # run the TUI app
just gui-dev    # run the GUI dev server
```

See `justfile` for all available commands (`just --list`).

## Rust

Run all Rust tests:

```bash
just test
# or: cargo test --workspace --all-targets
```

Run the TUI fake session:

```bash
just tui
# or: cargo run -p agent-tui
```

## GUI

Install frontend dependencies:

```bash
bun install
```

Run Vue unit tests:

```bash
just test-gui
# or: bun --filter agent-gui test
# plus: bun --filter agent-gui test:scripts
```

Run the Vite development server:

```bash
just gui-dev
# or: bun --filter agent-gui dev
```

The GUI dev server starts at port `1420` by default. If that port is already in use, Vite automatically chooses the next available port. Set `KAIROX_DEV_PORT` to start from a different port:

```bash
KAIROX_DEV_PORT=1530 bun --filter agent-gui dev
```

Run the Tauri desktop app in development mode:

```bash
just tauri-dev
# or: bun --filter agent-gui tauri:dev
```

The Tauri dev wrapper picks an available port, passes it to Vite, and overrides Tauri's `devUrl` for that run. This allows multiple local GUI dev instances to run side by side. When the `pilot` feature is enabled, the wrapper also adds the selected port to the dev identifier so each app gets a separate `tauri-pilot` socket:

```bash
bun --filter agent-gui tauri dev --features pilot
```

Kairox writes one runtime instance record per GUI/TUI process under `~/.kairox/runtime/instances/`. These JSON records include the process kind, PID, database file, and workspace path. Shutdown cleanup is best-effort because some desktop exits bypass Rust destructors; startup and instance queries prune dead-PID records before reporting any other running local instances so port, database, MCP/LSP, monitor, and workspace-file conflicts are easier to diagnose.

## Type synchronization

Kairox uses [tauri-specta](https://github.com/specta-rs/tauri-specta) to auto-generate Rust→TypeScript bindings for both commands and events. The generated files live under `apps/agent-gui/src/generated/` and **must not be edited by hand**.

- **Command types** (`commands.ts`): generated from `#[tauri::command]` functions registered in `apps/agent-gui/src-tauri/src/specta.rs`.
- **Event types** (`events.ts`): generated from `EventPayload`, `DomainEvent`, `TaskSnapshot`, `TaskGraphSnapshot`, `AgentRole`, `TaskState`, `MemoryScope`, etc. — domain types in `agent-core` and `agent-memory` annotated with `#[cfg_attr(feature = "specta", derive(specta::Type))]`.

Useful commands:

```bash
just gen-types     # regenerate commands.ts and events.ts after Rust changes
just check-types   # CI gate: regenerate and assert no diff under apps/agent-gui/src/generated/
```

The CI `type-sync` job enforces that generated bindings stay in sync.

## Integration & E2E tests

Beyond `just test` (cargo unit + integration), several focused recipes are available:

```bash
just test-tui         # TUI app logic integration tests (FakeModelClient + in-memory SQLite)
just test-fullstack   # full-stack runtime integration tests (tools, permissions, memory, persistence)
just test-mcp         # MCP-focused tests across agent-mcp / agent-tools / agent-config / agent-runtime
just test-e2e         # Playwright E2E tests for the GUI frontend (uses the Tauri IPC mock)
just test-e2e-headed  # E2E tests in headed mode for debugging
just test-e2e-ui      # Playwright UI mode
just test-all         # all layers: unit + integration + fullstack + GUI Vitest
```

E2E specs live in `apps/agent-gui/e2e/` and use a browser-side Tauri IPC mock. The top-level bridge is `apps/agent-gui/e2e/tauri-mock.js`; command handlers and mock state live under `apps/agent-gui/e2e/fixtures/tauri-mock/`. When you add or change a `#[tauri::command]` or an event the frontend listens to, update the matching fixture fragment so E2E tests keep passing.

## Configuration

Copy the example config and set up API keys:

```bash
mkdir -p .kairox
cp kairox.toml.example .kairox/config.toml
cp .env.example .env
# Edit .kairox/config.toml to choose model profiles
# Edit .env to set OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.
```

## Privacy Defaults

The initial runtime stores event envelopes and full fake-session content in SQLite during tests. Production configuration must default to `minimal_trace` when a real model or shell tool is configured.

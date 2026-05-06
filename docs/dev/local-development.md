# Local Development

## Prerequisites

- Rust stable toolchain (pinned by `rust-toolchain.toml`)
- Node.js 22+
- pnpm 10+
- [just](https://github.com/casey/just) task runner (`cargo install just` or `brew install just`)

## Quick start

```bash
pnpm install
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
pnpm install
```

Run Vue unit tests:

```bash
just test-gui
# or: pnpm --filter agent-gui run test
```

Run the Vite development server:

```bash
just gui-dev
# or: pnpm --filter agent-gui run dev
```

Run the Tauri desktop app in development mode:

```bash
just tauri-dev
# or: pnpm --filter agent-gui run tauri:dev
```

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
just test-e2e         # Playwright E2E tests for the GUI frontend (uses tauri-mock.js IPC mock)
just test-e2e-headed  # E2E tests in headed mode for debugging
just test-e2e-ui      # Playwright UI mode
just test-all         # all layers: unit + integration + fullstack + GUI Vitest
```

E2E specs live in `apps/agent-gui/e2e/` and use a browser-side Tauri IPC mock (`apps/agent-gui/e2e/tauri-mock.js`). When you add or change a `#[tauri::command]` or an event the frontend listens to, also update the mock so E2E tests keep passing.

## Configuration

Copy the example config and set up API keys:

```bash
cp kairox.toml.example kairox.toml
cp .env.example .env
# Edit kairox.toml to choose model profiles
# Edit .env to set OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.
```

## Privacy Defaults

The initial runtime stores event envelopes and full fake-session content in SQLite during tests. Production configuration must default to `minimal_trace` when a real model or shell tool is configured.

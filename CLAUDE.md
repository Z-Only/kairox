# Kairox — Claude Code Instructions

> This file is the entry point for Claude Code. For the full project guide, see [AGENTS.md](./AGENTS.md).

## Quick reference

- **Language**: Rust workspace + Vue 3 / TypeScript (Tauri 2)
- **Package manager**: pnpm only (never npm)
- **Lint & format**: `pnpm run lint`, `pnpm run format:check`
- **Test**: `cargo test --workspace --all-targets`
- **GUI test**: `pnpm --filter agent-gui run test`

## Crate map (dependency direction →)

| Crate         | Role                                     | Key trait/type                     |
| ------------- | ---------------------------------------- | ---------------------------------- |
| agent-core    | Domain types, events, facade             | `AppFacade`                        |
| agent-store   | SQLite event store + metadata            | `EventStore`                       |
| agent-memory  | Memory & context assembly                | `MemoryStore`                      |
| agent-models  | LLM adapters (OpenAI, Anthropic, Ollama) | `ModelClient`                      |
| agent-tools   | Tool registry & permissions              | `ToolRegistry`, `PermissionEngine` |
| agent-config  | TOML config, profile discovery           | `ProfileDef`                       |
| agent-runtime | Orchestrates agent loop                  | `LocalRuntime<S,M>`                |
| agent-tui     | Terminal UI (ratatui)                    | `App`                              |
| agent-gui     | Desktop app (Tauri + Vue), sessions      | `commands.rs` → Pinia stores       |

## Before starting work

1. Read [AGENTS.md](./AGENTS.md) for architecture, conventions, and pitfalls.
2. Run `pnpm install` (required after worktree creation for husky hooks).
3. Run `pnpm run format:check && pnpm run lint && cargo test --workspace --all-targets` to confirm a clean baseline.

## When adding features

1. Start from `agent-core` if new domain types/events are needed.
2. Follow dependency direction — never create reverse deps.
3. Add tests first: use `FakeModelClient` for runtime, in-memory SQLite for stores.
4. Wire to UIs last: Tauri commands for GUI, `app.rs` handlers for TUI.
5. Mirror new `EventPayload` variants in `apps/agent-gui/src/types/index.ts`.

## When bumping versions

Edit all 5 files in sync: `Cargo.toml`, `Cargo.lock` (via `cargo generate-lockfile`), `apps/agent-gui/package.json`, `apps/agent-gui/src-tauri/tauri.conf.json`, root `package.json`.

## Commit convention

Conventional Commits with scopes: `core`, `runtime`, `models`, `tools`, `memory`, `store`, `config`, `tui`, `gui`, `deps`, `ci`.

Examples: `feat(runtime): ...`, `fix(gui): ...`, `chore(deps): ...`

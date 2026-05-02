# Kairox — Copilot Instructions

## Project

Kairox is a local-first AI agent workbench: Rust workspace core + Tauri/Vue GUI + ratatui TUI.

## Language & tooling

- Rust workspace (edition 2021, resolver 2) + Vue 3 + TypeScript
- Package manager: **pnpm only** (never npm or yarn)
- Lint: `cargo clippy -D warnings`, ESLint, Stylelint
- Format: `cargo fmt`, Prettier
- Test: `cargo test --workspace --all-targets`, `vitest run`

## Crate structure & dependency direction

```
agent-core ← agent-runtime ← agent-tui / agent-gui (Tauri)
agent-core ← agent-store, agent-memory, agent-models, agent-tools, agent-config
agent-runtime → agent-memory, agent-store, agent-models, agent-tools, agent-config
```

Never create reverse dependencies. New domain types/events go in `agent-core` first.

## Key patterns

- **Workspace version**: all crates use `version.workspace = true` — never set `version` in individual crate Cargo.toml
- **Error handling**: each crate has its own `XxxError` enum using `thiserror::Error` and `type Result<T> = std::result::Result<T, XxxError>`
- **Trait boundaries**: `AppFacade`, `ModelClient`, `MemoryStore`, `EventStore`, `Tool` — prefer `Arc<dyn Trait>` or generics
- **Async**: Tokio + `async_trait` for async trait methods
- **Testing**: `FakeModelClient` for runtime tests, in-memory SQLite for store tests
- **Tauri IPC**: Rust `#[tauri::command]` in `commands.rs` → Vue calls `invoke()` → events via `app_handle.emit()` + `useTauriEvents.ts`

## Vue conventions

- Vue 3 Composition API (`<script setup lang="ts">`)
- Pinia stores in `apps/agent-gui/src/stores/`
- Composables in `apps/agent-gui/src/composables/`
- Types in `apps/agent-gui/src/types/`

## Commit format

Conventional Commits with scopes: `core`, `runtime`, `models`, `tools`, `memory`, `store`, `config`, `tui`, `gui`, `deps`, `ci`

## Version bump files (must stay in sync)

1. `Cargo.toml` (workspace.package.version)
2. `Cargo.lock` (run `cargo generate-lockfile`)
3. `apps/agent-gui/package.json`
4. `apps/agent-gui/src-tauri/tauri.conf.json`
5. `package.json` (root)

## Common mistakes to avoid

- Using `npm` instead of `pnpm`
- Setting `version` in individual crate Cargo.toml
- Hardcoding API keys (use `agent-config`'s `api_key_env`)
- Forgetting to update both Rust and TypeScript types when changing events
- Skipping `pnpm install` after creating a git worktree

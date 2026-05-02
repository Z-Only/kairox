# Kairox — AI Agent Instructions

This file provides project-specific guidance for AI coding assistants (Codex, Claude Code, Cursor, etc.).

## Project overview

Kairox is a **local-first AI agent workbench** with a shared Rust core, a terminal UI (ratatui), and a Tauri + Vue desktop GUI. The architecture follows an event-sourced, facade-driven design where all crate boundaries are trait-based for testability.

## Architecture & crate map

```
┌─────────────────────────────────────────────────────┐
│  User Interfaces                                     │
│  ┌──────────────┐  ┌──────────────────────────────┐ │
│  │  agent-tui   │  │  agent-gui (Tauri 2 + Vue 3) │ │
│  │  (ratatui)   │  │  Tauri commands ↔ Vue stores  │ │
│  └──────┬───────┘  └──────────────┬───────────────┘ │
└─────────┼─────────────────────────┼─────────────────┘
          │                         │
          ▼                         ▼
┌─────────────────────────────────────────────────────┐
│  agent-core (facade, domain types, events, IDs)     │
│  └── AppFacade trait — the primary integration point│
└──────────────────────────┬──────────────────────────┘
                           │
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
  │agent-runtime │ │agent-memory  │ │agent-store   │
  │LocalRuntime  │ │MemoryStore   │ │EventStore    │
  │agents, tasks │ │ContextAsmblr │ │SqliteEventSt. │
  └──────┬───────┘ └──────────────┘ └──────────────┘
         │
    ┌────┴─────────────┐
    ▼                  ▼
┌──────────┐   ┌──────────────┐   ┌──────────────┐
│agent-    │   │agent-models  │   │agent-config  │
│tools     │   │ModelClient   │   │ProfileDef    │
│Perms,Reg│   │Router,LLMs   │   │Discovery,Load│
└──────────┘   └──────────────┘   └──────────────┘
```

### Crate responsibilities

| Crate             | Purpose                                                                          | Key types                                                                                                  |
| ----------------- | -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| **agent-core**    | Shared domain types, event definitions, facade trait, IDs, projections           | `AppFacade`, `DomainEvent`, `EventPayload`, `SessionId`, `WorkspaceId`, `TraceEntry`, `PermissionDecision` |
| **agent-runtime** | Orchestrates the agent loop, manages sessions, wires tools/memory/permissions    | `LocalRuntime<S, M>`, `PlannerAgent`, `WorkerAgent`, `ReviewerAgent`, `TaskGraph`                          |
| **agent-models**  | Model provider abstraction (OpenAI-compatible, Anthropic, Ollama, Fake)          | `ModelClient` trait, `ModelRequest`, `ModelRouter`, `ModelProfile`                                         |
| **agent-tools**   | Tool registry, permission engine, built-in tools (shell, fs, patch, search, MCP) | `ToolRegistry`, `PermissionEngine`, `Tool` trait, `PermissionMode`, `ToolRisk`                             |
| **agent-memory**  | Durable/user/workspace/session-scoped memory, context assembly with tiktoken     | `MemoryStore` trait, `SqliteMemoryStore`, `ContextAssembler`, `MemoryMarker`, `MemoryScope`                |
| **agent-store**   | SQLite-backed event store (append-only)                                          | `EventStore` trait, `SqliteEventStore`                                                                     |
| **agent-config**  | TOML config loading, model profile discovery, API key resolution                 | `ProfileDef`, `load_from_str`, `build_router`                                                              |
| **agent-tui**     | Interactive terminal UI (ratatui three-panel: sessions, chat, trace)             | `App`, `ChatPanel`, `SessionsPanel`, `TracePanel`, `PermissionModal`                                       |
| **agent-gui**     | Tauri 2 + Vue 3 desktop app (Tauri commands → Vue Pinia stores → components)     | `commands.rs`, `GuiState`, Vue: `session.ts` store, `useTraceStore.ts` composable                          |

## Coding conventions

### Rust

- **Versioning**: All crates share `version.workspace = true` from the root `Cargo.toml` `[workspace.package]`. **Never set `version` in individual crate Cargo.toml files.**
- **Error handling**: Each crate defines its own `XxxError` enum using `thiserror::Error`. Use `thiserror(transparent)` for wrapped errors. Each crate has a `type Result<T> = std::result::Result<T, XxxError>`.
- **Trait-based boundaries**: Crate dependencies go through traits (`AppFacade`, `ModelClient`, `MemoryStore`, `EventStore`, `Tool`, `ToolProvider`). Prefer `Arc<dyn Trait>` or generic parameters for composition.
- **Async runtime**: Tokio. Use `async_trait` for async trait methods.
- **Testing**: Unit tests in `#[cfg(test)] mod tests` within the module. Integration tests in `crates/<crate>/tests/`. Use `FakeModelClient` for runtime tests that need a model.
- **Naming**: IDs are typed newtypes (`SessionId`, `WorkspaceId`, etc.) with `serde` support. Domain events use `DomainEvent` + `EventPayload` enum pattern.
- **Clippy**: `cargo clippy --workspace --all-targets --all-features -- -D warnings` must pass with zero warnings.
- **Workspace dependencies**: Shared dependency versions are centralized in the root `Cargo.toml` `[workspace.dependencies]`. Reference them as `dep.workspace = true` in crate Cargo.toml.

### TypeScript / Vue

- **Framework**: Vue 3 Composition API + TypeScript (`<script setup lang="ts">`)
- **State management**: Pinia stores (`apps/agent-gui/src/stores/`). Composables in `composables/`.
- **Types**: Centralized in `apps/agent-gui/src/types/`. Mirror Rust event types for Tauri IPC.
- **Testing**: Vitest with `vitest/globals`. Test files colocated as `*.test.ts`.
- **Style**: Prettier + ESLint + Stylelint. See lint-staged config for auto-fix rules.

### Tauri IPC pattern

The GUI follows this pattern:

1. Rust `commands.rs` defines `#[tauri::command]` functions that call `AppFacade` methods
2. `lib.rs` registers all commands and manages `GuiState` (holds `Arc<LocalRuntime<...>>`)
3. Vue frontend calls `invoke("command_name", { args })` via `@tauri-apps/api`
4. Events flow Rust→Vue via `app_handle.emit("event-name", payload)` and `useTauriEvents.ts` listener

### Permission system

`PermissionMode` controls tool execution behavior:

- `ReadOnly` — only read operations allowed
- `Suggest` — UI prompts for approval (default)
- `Agent` — agent decides within policy
- `Autonomous` — all operations allowed
- `Interactive` — per-request approval with pending state

### Memory protocol

The LLM uses `<memory>` tags in responses to propose memories:

- `<memory scope="session">` — temporary, auto-accepted
- `<memory scope="user" key="...">` — user preference, requires approval
- `<memory scope="workspace" key="...">` — project setting, requires approval

Markers are parsed by `agent-memory::extract_memory_markers`, stripped from display output, and stored via `MemoryStore`.

## Project structure

```
kairox/
├── Cargo.toml              # Workspace root (shared version + deps)
├── Cargo.lock
├── package.json            # pnpm tooling root (format, lint, prepare)
├── pnpm-lock.yaml
├── cliff.toml              # git-cliff changelog config
├── commitlint.config.cjs   # Conventional Commits enforcement
├── scripts/
│   ├── release.sh          # Automated release script
│   └── prepare.cjs         # Husky hook setup (worktree-aware)
├── crates/
│   ├── agent-core/         # Domain types, facade, events, IDs
│   ├── agent-runtime/      # LocalRuntime, agents, task graph
│   ├── agent-models/       # ModelClient trait + adapters
│   ├── agent-tools/        # Tool registry, permission engine
│   ├── agent-memory/       # Memory store, context assembler
│   ├── agent-store/        # SQLite event store
│   ├── agent-config/       # Config loading, profile discovery
│   └── agent-tui/          # ratatui TUI app
├── apps/
│   └── agent-gui/          # Tauri 2 + Vue 3 desktop app
│       ├── src/            # Vue frontend
│       │   ├── components/ # ChatPanel, TraceTimeline, PermissionPrompt, etc.
│       │   ├── stores/     # Pinia stores (session.ts)
│       │   ├── composables/# useTauriEvents, useTraceStore
│       │   └── types/      # TypeScript type definitions
│       ├── src-tauri/      # Rust Tauri backend
│       │   ├── src/        # commands.rs, app_state.rs, event_forwarder.rs, lib.rs
│       │   ├── Cargo.toml  # version.workspace = true
│       │   └── tauri.conf.json
│       ├── package.json
│       └── vite.config.ts
├── docs/
│   ├── dev/                # Local development guide
│   ├── github/             # Discussion templates, labels
│   └── superpowers/        # OpenSpec plans & specs
└── .github/workflows/      # CI, release-build, dependabot-auto-merge
```

## Commit conventions

Conventional Commits are enforced via commitlint + husky. Use these scopes:

| Scope     | When to use                               |
| --------- | ----------------------------------------- |
| `core`    | Changes to `agent-core`                   |
| `runtime` | Changes to `agent-runtime`                |
| `models`  | Changes to `agent-models`                 |
| `tools`   | Changes to `agent-tools`                  |
| `memory`  | Changes to `agent-memory`                 |
| `store`   | Changes to `agent-store`                  |
| `config`  | Changes to `agent-config`                 |
| `tui`     | Changes to `agent-tui`                    |
| `gui`     | Changes to `apps/agent-gui` (Rust or Vue) |
| `deps`    | Dependency updates (Cargo or npm)         |
| `ci`      | CI/CD workflow changes                    |

Examples:

- `feat(runtime): add scheduler retry policy`
- `fix(gui): handle empty trace state`
- `refactor(memory): extract keyword scoring into separate module`
- `chore(deps): bump reqwest to 0.12`

## Git worktrees

This project uses git worktrees for isolated branch development. After creating a worktree, always run `pnpm install` to set up husky hooks:

```bash
git worktree add ../kairox-<branch> -b <branch> main
cd ../kairox-<branch>
pnpm install   # triggers prepare.cjs which links husky hooks
```

The `prepare.cjs` script detects worktrees and creates a symlink from `GIT_DIR/.husky` to the worktree's `.husky` directory so that pre-commit and commit-msg hooks fire correctly.

## Local verification

Run before opening a PR or pushing to main:

```bash
pnpm run format:check
pnpm run lint
cargo test --workspace --all-targets
```

Pre-commit hooks (husky + lint-staged) automatically run on staged files:

- `*.rs` → `cargo fmt --all`
- `*.{json,md}` → `prettier --write`
- `apps/agent-gui/**/*.{ts,tsx,js,jsx,vue}` → `prettier --write` + `eslint --fix`
- `apps/agent-gui/src/**/*.{vue,css,scss,sass,less}` → `prettier --write` + `stylelint --fix`

## Version bumping

When bumping the version for a release, edit these files (all must stay in sync):

1. **`Cargo.toml`** — `workspace.package.version`
2. **`Cargo.lock`** — run `cargo generate-lockfile` to update all crate versions
3. **`apps/agent-gui/package.json`** — `"version"` field
4. **`apps/agent-gui/src-tauri/tauri.conf.json`** — `"version"` field
5. **`package.json`** (root) — `"version"` field

Do NOT edit `version` in individual crate `Cargo.toml` files — they inherit from the workspace.

## Release flow

Use `scripts/release.sh <version>` to publish a release:

```bash
scripts/release.sh 0.7.0
```

The script runs checks, verifies the GUI build, generates `CHANGELOG.md` with git-cliff, commits it, creates the tag, and pushes.

### Manual release steps (if not using the script)

1. Bump version in all config files (see above)
2. Commit the version bump: `git commit -m "chore(release): bump version to X.Y.Z"`
3. Run `git cliff --tag vX.Y.Z -o CHANGELOG.md`
4. Commit the changelog: `git commit -m "chore(release): update CHANGELOG for vX.Y.Z"`
5. Create and push the tag: `git tag -fa vX.Y.Z -m "vX.Y.Z" && git push origin main && git push origin vX.Y.Z -f`

**Always commit an updated `CHANGELOG.md` before pushing the release tag.** The tag should point to a commit that includes the changelog update.

### How git-cliff works

- `cliff.toml` at the repo root configures the changelog format and commit grouping
- Commits are grouped into Features, Bug Fixes, Performance, Documentation, Testing, Refactor, Styling, CI, Dependencies, and Miscellaneous
- `chore(release):` commits are automatically excluded from the changelog
- GitHub Actions also runs git-cliff to generate Release Notes on the GitHub Release page

## CI

- **CI** (`ci.yml`) runs on push to `main` and on pull requests: format check, lint, cargo test, TUI build, GUI web build
- **Release Build** (`release-build.yml`) runs on `v*` tags: publishes release notes via git-cliff, builds TUI binaries for all platforms, builds Tauri desktop bundles for all platforms
- **Dependabot Auto Merge** automatically merges passing Dependabot PRs for npm, Cargo, and GitHub Actions dependencies

## AI coding guidelines

### tauri-specta for command IPC type generation

[tauri-specta](https://github.com/specta-rs/tauri-specta) auto-generates TypeScript types from `#[tauri::command]` return types and arguments, eliminating drift for **command IPC**. These bindings are generated into `apps/agent-gui/src/generated/commands.ts`.

**When to regenerate**: after adding/modifying/removing any `#[tauri::command]` function or its parameter/return types:

```bash
just gen-types
```

**What specta covers**: command function signatures and their return types (`WorkspaceInfoResponse`, `SessionInfoResponse`, `MemoryEntryResponse`, `ProfileInfo`, etc.).

**What specta does NOT cover**: `EventPayload` types that flow through `app_handle.emit()` (broadcast events). These are not command return values, so specta cannot generate types for them. Rust↔TypeScript EventPayload sync is enforced by `just check-types` (and the `type-sync` CI job).

**When adding a new command**:

1. Add `#[specta::specta]` attribute to the command function
2. Add the command to the `collect_commands![]` macro in `src/specta.rs`
3. Add any new response types with `#[derive(specta::Type)]`
4. Run `just gen-types` to regenerate the TypeScript bindings

### When adding a new feature

1. **Start from `agent-core`** if the feature introduces new domain types, events, or facade methods. Define the types and trait changes there first.
2. **Implement in the appropriate crate** following the dependency direction: core → store/memory/config → models/tools → runtime → tui/gui. Never create reverse dependencies.
3. **Add tests first**: use `FakeModelClient` for runtime tests, `SqliteEventStore`/:`SqliteMemoryStore` with in-memory SQLite for persistence tests.
4. **Wire up to UIs last**: add Tauri commands in `commands.rs` for GUI, add components/handlers in `app.rs` for TUI.
5. **Update types**: if adding new event variants, update `EventPayload` and mirror in `apps/agent-gui/src/types/index.ts`.

### When fixing a bug

1. Write a failing test that reproduces the bug.
2. Fix the code, verify the test passes.
3. Run the full verification suite before committing.

### When modifying the GUI

- Vue components go in `apps/agent-gui/src/components/`
- Pinia stores go in `apps/agent-gui/src/stores/`
- Composables go in `apps/agent-gui/src/composables/`
- TypeScript types go in `apps/agent-gui/src/types/`
- Always update the corresponding Rust `#[tauri::command]` in `commands.rs` if the IPC surface changes
- Use `useTauriEvents.ts` for real-time Rust→Vue event streaming

### Common pitfalls

- **Don't add crate-level `version`**: all crates use `version.workspace = true`
- **Don't skip `cargo clippy`**: CI denies warnings
- **Don't use `npm`**: this project uses `pnpm` exclusively
- **Don't forget `pnpm install` after creating a worktree**: husky hooks won't fire otherwise
- **Don't hardcode API keys**: use `agent-config`'s `api_key_env` to reference environment variables
- **Don't forget to update both Rust and TypeScript types** when changing the event/domain model

## Privacy defaults

The initial runtime stores event envelopes and full fake-session content in SQLite during tests. Production configuration must default to `minimal_trace` when a real model or shell tool is configured.

## Quick command reference (`just`)

A `justfile` is provided for common tasks. Install with `cargo install just` or `brew install just`.

| Command                   | Description                              |
| ------------------------- | ---------------------------------------- |
| `just check`              | Full CI gate: format check + lint + test |
| `just fmt-check`          | Check formatting (Rust + web)            |
| `just lint`               | Run clippy + eslint + stylelint          |
| `just test`               | Run all Rust tests                       |
| `just test-gui`           | Run GUI (Vitest) tests                   |
| `just fmt`                | Auto-format all code                     |
| `just tui`                | Run the TUI app                          |
| `just gui-dev`            | Run GUI dev server (Vite)                |
| `just tauri-dev`          | Run Tauri desktop app in dev mode        |
| `just bump-version X.Y.Z` | Bump version in all config files         |
| `just check-types`        | Verify Rust↔TypeScript EventPayload sync |
| `just worktree <name>`    | Create a git worktree with pnpm install  |

## Common workflow recipes

### Adding a new event type

1. **Add the variant** to `EventPayload` in `crates/agent-core/src/events.rs`
2. **Add the match arm** in `EventPayload::event_type()` (same file)
3. **Mirror the type** in `apps/agent-gui/src/types/index.ts` as a TypeScript discriminated union variant
4. **Run `just check-types`** to verify Rust and TS are in sync
5. **Emit the event** from the appropriate place in `agent-runtime` (e.g., `facade_runtime.rs`)
6. **Handle the event** in the UI:
   - TUI: update the relevant component in `crates/agent-tui/src/components/`
   - GUI: update `useTraceStore.ts` or the relevant Pinia store/composable

### Adding a new tool

1. **Implement the `Tool` trait** in a new module under `crates/agent-tools/src/` (e.g., `my_tool.rs`)
2. **Register the tool** in `crates/agent-tools/src/registry.rs` via `BuiltinProvider`
3. **Define risk level** in `crates/agent-tools/src/permission.rs` using `ToolRisk`
4. **Add tests** in `crates/agent-tools/src/my_tool.rs` under `#[cfg(test)]`
5. **Wire into runtime** in `crates/agent-runtime/src/facade_runtime.rs` — register the tool in the `ToolRegistry`
6. **Update permission UI** if the tool has a new `ToolEffect` variant (TUI: `permission_modal.rs`, GUI: `PermissionPrompt.vue`)

### Adding a new model provider

1. **Implement `ModelClient` trait** in a new module under `crates/agent-models/src/` (e.g., `my_provider.rs`)
2. **Add a config struct** (e.g., `MyProviderConfig`) with `base_url`, `api_key_env`, etc.
3. **Register in `ModelRouter`** via `crates/agent-models/src/router.rs`
4. **Add profile entry** in `crates/agent-config/src/builder.rs` to map provider string → client constructor
5. **Update `ProfileDef` docs** in `crates/agent-config/src/lib.rs` and `kairox.toml.example`
6. **Add tests** using the existing `FakeModelClient` pattern as a reference

### Adding a new GUI component

1. **Create the Vue SFC** in `apps/agent-gui/src/components/` with `<script setup lang="ts">`
2. **If it needs Tauri IPC**: add a `#[tauri::command]` in `apps/agent-gui/src-tauri/src/commands.rs`, register it in `lib.rs`
3. **If it needs reactive state**: create a Pinia store in `apps/agent-gui/src/stores/` or a composable in `composables/`
4. **If it handles events**: use `useTauriEvents.ts` to listen for `DomainEvent` payloads
5. **Add types** to `apps/agent-gui/src/types/` as needed
6. **Import and use** the component in `App.vue` or the relevant parent component

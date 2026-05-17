# Kairox — Claude Code Instructions

> This file is the entry point for Claude Code. For the full project guide, see [AGENTS.md](./AGENTS.md).

## Quick reference

- **Language**: Rust workspace + Vue 3 / TypeScript (Tauri 2)
- **Package manager**: Bun only (never npm, pnpm, or yarn)
- **Lint & format**: `bun run lint`, `bun run format:check`
- **Test**: `cargo test --workspace --all-targets`
- **GUI test**: `bun --filter agent-gui test`

## Crate map (dependency direction →)

| Crate         | Role                                                                                            | Key trait/type                                        |
| ------------- | ----------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| agent-core    | Domain types, events, facade, build info                                                        | `AppFacade`, `EventPayload`, `TaskSnapshot`           |
| agent-store   | SQLite event store + metadata                                                                   | `EventStore`, `SqliteEventStore`                      |
| agent-memory  | Memory, context assembly, and compaction                                                        | `MemoryStore`, `ContextAssembler`, `ContextCompactor` |
| agent-models  | LLM adapters + model metadata/context windows                                                   | `ModelClient`, `ModelRouter`, `ModelRegistry`         |
| agent-tools   | Tool registry & permissions, built-in tools                                                     | `ToolRegistry`, `PermissionEngine`, `Tool`            |
| agent-mcp     | MCP client, transports (stdio/sse), lifecycle, marketplace catalog                              | `McpClient`, `Transport`, `ServerLifecycle`           |
| agent-skills  | Native skills system — reusable prompt/tool/workflow capabilities, config-driven discovery      | `SkillRegistry`, `SkillDef`, `SkillFrontmatter`       |
| agent-config  | TOML config, profile discovery, `.kairox/` discovery, instructions, skills/MCP config           | `ProfileDef`, `build_router`                          |
| agent-runtime | Agent loop, context budgets, compaction, model switching, DAG execution, multi-agent strategies | `LocalRuntime<S,M>`, `DagExecutor`, `AgentStrategy`   |
| agent-tui     | Terminal UI (ratatui)                                                                           | `App`                                                 |
| agent-gui     | Desktop app (Tauri + Vue), sessions, MCP UI, instructions, skills, workspaces                   | `commands.rs` → Pinia stores                          |

> Built-in tools shipped by `agent-tools`: `shell` (`ShellExecTool`), `fs.read`, `fs.write`, `fs.list`, `patch` (`PatchApplyTool`), `search` (`RipgrepSearchTool`). External tools come from MCP servers via `McpToolAdapter`.

## Before starting work

1. Read [AGENTS.md](./AGENTS.md) for architecture, conventions, and pitfalls.
2. Run `bun install` (required after worktree creation for husky hooks).
3. Run `bun run format:check && bun run lint && cargo test --workspace --all-targets` to confirm a clean baseline.

## When adding features

1. Start from `agent-core` if new domain types/events are needed.
2. Follow dependency direction — never create reverse deps.
3. Add tests first: use `FakeModelClient` for runtime, in-memory SQLite for stores.
4. Wire to UIs last: Tauri commands for GUI, `app.rs` handlers for TUI.
5. If model/profile behavior changes, update model metadata/context-window tests and verify mid-session model switching still respects budget limits.
6. After changing any `#[tauri::command]` or `EventPayload`/domain type, run `just gen-types` to regenerate `apps/agent-gui/src/generated/{commands,events}.ts` (do not edit those files manually).
7. If you add new IPC commands or events, also update the Playwright mock at `apps/agent-gui/e2e/tauri-mock.js`.

## When bumping versions

Edit all 5 files in sync: `Cargo.toml`, `Cargo.lock` (via `cargo generate-lockfile`), `apps/agent-gui/package.json`, `apps/agent-gui/src-tauri/tauri.conf.json`, root `package.json`.

## Commit convention

Conventional Commits with scopes: `core`, `runtime`, `models`, `tools`, `memory`, `store`, `config`, `mcp`, `skills`, `tui`, `gui`, `deps`, `ci`.

Examples: `feat(runtime): ...`, `fix(gui): ...`, `feat(mcp): ...`, `chore(deps): ...`

## Useful test recipes

- `just test` — all Rust unit + integration tests
- `just test-tui` — TUI app logic integration tests
- `just test-fullstack` — full-stack runtime integration tests
- `just test-mcp` — MCP-focused tests across `agent-mcp`, `agent-tools`, `agent-config`, `agent-runtime`
- `just test-e2e` — Playwright E2E tests for the GUI frontend (uses Tauri IPC mock)
- `just test-pilot` — real Tauri desktop E2E scenarios (requires `tauri-pilot-cli`; use `xvfb-run -a` on Linux)
- `just test-live` — live GitHub Models smoke test (self-skips without `GITHUB_TOKEN`)
- `just test-all` — unit + integration + fullstack + GUI Vitest

## Common pitfalls

- Don't use `npm`, `pnpm`, or `yarn` for project package management; use Bun.
- Don't set `version` in individual crate `Cargo.toml` — they inherit from `[workspace.package]`.
- Don't edit files under `apps/agent-gui/src/generated/` by hand.
- After creating a worktree, always run `bun install` so husky hooks fire.
- Register new Tauri commands in **both** `generate_handler!` (in `lib.rs`) **and** `collect_commands!` (in `src/specta.rs`).
- Keep context-budget, compaction, and model-switching behavior in sync across `agent-core`, `agent-runtime`, `agent-memory`, `agent-models`, TUI, and GUI when touching session/model state.

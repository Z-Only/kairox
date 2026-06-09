# Kairox — AI Agent Instructions

This file provides project-specific guidance for AI coding assistants (Codex, Claude Code, Cursor, etc.).

## Project overview

Kairox is a **local-first AI agent workbench** with a shared Rust core, a terminal UI (ratatui), and a Tauri + Vue desktop GUI. The architecture follows an event-sourced, facade-driven design where all crate boundaries are trait-based for testability. It includes native skills and plugins for reusable prompt, tool, and workflow capabilities, with config-driven discovery and GUI settings management.

## Architecture & crate map

```
┌──────────────────────────────────────────────────────────────────────┐
│  User Interfaces & Consumers                                          │
│  ┌──────────────┐  ┌──────────────────────────────┐  ┌────────────┐ │
│  │  agent-tui   │  │  agent-gui (Tauri 2 + Vue 3) │  │ agent-eval │ │
│  │  (ratatui)   │  │  Tauri commands ↔ Vue stores  │  │(kairox-eval)│ │
│  └──────┬───────┘  └──────────────┬───────────────┘  └─────┬──────┘ │
└─────────┼─────────────────────────┼─────────────────────────┼────────┘
          │                         │                         │
          ▼                         ▼                         ▼
┌─────────────────────────────────────────────────────────────────────┐
│  agent-core (facade, domain types, events, IDs)                      │
│  └── AppFacade trait — the primary integration point                 │
└──────────────────────────────┬───────────────────────────────────────┘
                               │
          ┌────────────────────┼────────────────────┐
          ▼                    ▼                    ▼
  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
  │agent-runtime │     │agent-memory  │     │agent-store   │
  │LocalRuntime  │     │MemoryStore   │     │EventStore    │
  │agents, tasks │     │ContextAsmblr │     │SqliteEventSt. │
  └──────┬───────┘     └──────────────┘     └──────────────┘
         │
    ┌────┴─────────────┐
    ▼                  ▼
┌──────────┐   ┌──────────────┐   ┌──────────────┐   ┌──────────────┐   ┌──────────┐   ┌──────────────┐   ┌──────────────┐
│agent-    │   │agent-models  │   │agent-config  │   │agent-mcp     │   │agent-lsp │   │agent-skills  │   │agent-plugins │
│tools     │   │ModelClient   │   │ProfileDef    │   │McpClient     │   │LspClient │   │SkillRegistry │   │PluginManifest│
│Perms,Reg│   │Router,LLMs   │   │Discovery,Load│   │Transport,Lif.│   │DapClient │   │Frontmatter   │   │Inventory     │
└──────────┘   └──────────────┘   └──────────────┘   └──────────────┘   └──────────┘   └──────────────┘   └──────────────┘
```

### Crate responsibilities

| Crate             | Purpose                                                                                                                                                                                                                                                                                                              | Key types                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **agent-core**    | Shared domain types, event definitions, facade trait, IDs, projections, build info, trajectory DTOs, autonomous task types, advisor types                                                                                                                                                                            | `AppFacade`, `DomainEvent`, `EventPayload`, `SessionId`, `WorkspaceId`, `TraceEntry`, `PermissionDecision`, `TaskSnapshot`, `TaskGraphSnapshot`, `AgentRole`, `TaskState`, `BuildInfo`, `TrajectoryId`, `TrajectoryStep`, `TrajectoryMeta`, `TrajectoryOutcome`, `AutonomousFacade`, `AutonomousTaskId`, `AdvisorMode`, `AdvisorVerdict`                                                                                                                                                                                                                                                                                                                                                      |
| **agent-runtime** | Orchestrates the agent loop, session actors, context budgets, race-free turn-end compaction, model switching, configurable agent settings, multi-agent strategies, MCP server lifecycle, permissions, DAG turn scheduling, advisor self-reflection                                                                   | `LocalRuntime<S, M>`, `SessionActor`, `SessionExecutionRuntime`, `PlannerAgent`, `WorkerAgent`, `ReviewerAgent`, `AgentStrategy`, `DagExecutor`, `TaskGraph`, `McpServerManager`, `advisor::review_tool_calls`, `advisor::should_review`, context budget helpers                                                                                                                                                                                                                                                                                                                                                                                                                              |
| **agent-models**  | Model provider abstraction (OpenAI-compatible, Anthropic, Ollama, Fake) with model metadata/context-window support                                                                                                                                                                                                   | `ModelClient` trait, `ModelRequest`, `ModelRouter`, `ModelProfile`, `ModelRegistry`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| **agent-tools**   | Tool registry, orthogonal Approval × Sandbox policy engine, built-in tools (`shell.exec`, `fs.read`, `fs.write`, `fs.list`, `patch.apply`, `search.ripgrep`, `monitor.start/list/stop`, `browser.action`, `browser.batch`, `computer.use`), LSP/DAP tool providers, MonitorRegistry for background process lifecycle | `ToolRegistry`, `PolicyEngine`, `ApprovalPolicy`, `SandboxPolicy`, `PolicyDecision`, `PolicyRisk`, `PermissionEngine` (thin wrapper), `Tool` trait, `ToolRisk`, `McpToolAdapter`, `LspToolProvider`, `DapToolProvider`, `MonitorRegistry`, `MonitorInfo`, `BrowserTool`, `BrowserBatchTool`, `ComputerUseTool`                                                                                                                                                                                                                                                                                                                                                                                |
| **agent-mcp**     | MCP (Model Context Protocol) client, stdio/SSE/Streamable HTTP transports, server lifecycle, health checks, protocol types, discovery cache, marketplace catalog (built-in + remote sources)                                                                                                                         | `McpClient`, `Transport` trait, `StdioTransport`, `SseTransport`, `StreamableHttpTransport`, `ServerLifecycle`, `McpServerDef`, `McpTransportDef`, `McpError`, `CatalogEntry`, `CatalogSource`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| **agent-lsp**     | LSP and DAP client implementations, JSON-RPC transport, server lifecycle management for code intelligence and debugging integration                                                                                                                                                                                  | `LspClient`, `DapClient`, `LspServerDef`, `DapServerDef`, `LspServerLifecycle`, `DapServerLifecycle`, `ServerStatus`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| **agent-skills**  | Native skills system — reusable prompt, tool, and workflow capabilities, frontmatter parsing, skill registry, GUI settings, config-driven discovery                                                                                                                                                                  | `SkillRegistry`, `SkillDef`, `SkillFrontmatter`, `SkillScope`, `SkillSettings`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| **agent-plugins** | Plugin manifest and inventory parsing for plugin-provided skills, tools, hooks, and MCP servers                                                                                                                                                                                                                      | `PluginManifest`, plugin inventory and settings helpers                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| **agent-sdk**     | Embeddable SDK exposing the Kairox runtime as a programmatic API for external harnesses, CI/CD pipelines, and custom UIs                                                                                                                                                                                             | `KairoxSdk`, `SdkBuilder`, `SdkSession`, `MessageStream`, `StreamEvent`, `CollectedResponse`, `SdkHook` trait, `HookAction`, `SdkConfig`, `SdkApprovalPolicy`, `SdkSandboxPolicy`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **agent-memory**  | Durable/user/workspace/session-scoped memory, context assembly with tiktoken, multimodal image pruning, and prompt compaction support                                                                                                                                                                                | `MemoryStore` trait, `SqliteMemoryStore`, `ContextAssembler`, `MemoryMarker`, `MemoryScope`, `ContextCompactor`, `ImagePruningStrategy`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| **agent-store**   | SQLite-backed event store (append-only), metadata tables for workspace/session tracking, and trajectory persistence                                                                                                                                                                                                  | `EventStore` trait, `SqliteEventStore`, `SessionMeta`, metadata repos, `TrajectoryStore`, `SqliteTrajectoryStore`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **agent-config**  | TOML config loading, model profile discovery, API key resolution, `.kairox/` project config discovery, skills config, instruction config                                                                                                                                                                             | `ProfileDef`, `load_from_str`, `build_router`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **agent-eval**    | Headless evaluation harness (binary `kairox-eval`) for replaying JSONL scenarios with tag filtering, fail-fast execution, summaries, reports, and scenario expectations                                                                                                                                              | `kairox-eval` binary, `EvalScenario`, `EvalExpectation`, `EvalRunOptions`, `EvalHarness`, `EvalResult`, `EvalSummary`, `EvalReport`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| **agent-tui**     | Interactive terminal UI (ratatui three-panel: sessions, chat, trace) with build-info banner                                                                                                                                                                                                                          | `App`, `ChatPanel`, `SessionsPanel`, `TracePanel`, `PermissionModal`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| **agent-gui**     | Tauri 2 + Vue 3 desktop app; Rust backend crate is `agent-gui-tauri` under `apps/agent-gui/src-tauri`                                                                                                                                                                                                                | `commands.rs`, `GuiState`, `event_forwarder.rs`, `specta.rs`, `tauri_plugin_updater`; Vue stores: `session.ts`, `taskGraph.ts`, `agents.ts`, `mcp.ts`, `memory.ts`, `catalog.ts`, `skills.ts`; components: `ChatPanel.vue`, `InstructionsSettingsPane.vue`, `TaskSteps.vue`, `TaskNode.vue`, `TraceTimeline.vue`, `PermissionPrompt.vue`, `ChatPermissionItem.vue`, `ChatToolCallItem.vue`, `ChatCompactionItem.vue`, `MemoryBrowser.vue`, `McpServerManager.vue`, `McpStatusIndicator.vue`, `SessionsSidebar.vue`, `StatusBar.vue`, `NotificationToast.vue`, `ConfirmDialog.vue`, `marketplace/{CatalogList,CatalogCard,CatalogDetail,InstalledList,InstallProgress,RuntimeMissingHint}.vue` |

## Coding conventions

### Rust

- **Versioning**: All crates share `version.workspace = true` from the root `Cargo.toml` `[workspace.package]`. **Never set `version` in individual crate Cargo.toml files.**
- **Error handling**: Each crate defines its own `XxxError` enum using `thiserror::Error`. Use `thiserror(transparent)` for wrapped errors. Each crate has a `type Result<T> = std::result::Result<T, XxxError>`.
- **Trait-based boundaries**: Crate dependencies go through traits (`AppFacade`, `ModelClient`, `MemoryStore`, `EventStore`, `Tool`, `ToolProvider`). Prefer `Arc<dyn Trait>` or generic parameters for composition.
- **Async runtime**: Tokio. Use `async_trait` for async trait methods.
- **Testing**: Small crate-local tests live as sibling files such as `src/foo_tests.rs`, loaded from the owning module with `#[cfg(test)]` and `#[path = "foo_tests.rs"] mod tests;` so they can exercise private helpers without bloating production modules. Cross-module or public API behavior goes in `crates/<crate>/tests/*.rs`; larger suites may use `tests/<suite>/main.rs` plus `support` modules and `tests/fixtures/`. Use `FakeModelClient` or `ToolCallingModel` for runtime tests that need a model, and in-memory SQLite stores for persistence tests. Live provider coverage must stay behind the `live-model-tests` feature and self-skip without credentials.
- **Naming**: IDs are typed newtypes (`SessionId`, `WorkspaceId`, etc.) with `serde` support. Domain events use `DomainEvent` + `EventPayload` enum pattern.
- **Clippy**: `cargo clippy --workspace --all-targets --all-features -- -D warnings` must pass with zero warnings.
- **Workspace dependencies**: Shared dependency versions are centralized in the root `Cargo.toml` `[workspace.dependencies]`. Reference them as `dep.workspace = true` in crate Cargo.toml.

### TypeScript / Vue

- **Framework**: Vue 3 Composition API + TypeScript (`<script setup lang="ts">`)
- **State management**: Pinia setup-stores (`defineStore('name', () => { /* state, getters, actions */ })`) under `apps/agent-gui/src/stores/`. Composables in `composables/`. Use `useXxxStore()` + `storeToRefs()` in consumers.
- **Routing**: vue-router with `createWebHashHistory()`. Route table at `apps/agent-gui/src/router/routes.ts`. Workbench routes are nested: `/workbench/:sessionId?`.
- **i18n**: vue-i18n v9 (composition API mode). Locale messages under `apps/agent-gui/src/locales/{en,zh-CN}.json`. Only common copy (`common.*`, `nav.*`, `settings.*`, `notifications.*`, `status.*`) is translated; per-feature strings stay inline.
- **UI library**: The GUI does not use a full visual UI kit. It uses native HTML elements + CSS custom properties (`apps/agent-gui/src/styles/theme.css`) + a shared CSS class library (`apps/agent-gui/src/styles/components.css`). Headless accessibility primitives such as `reka-ui` may be used only behind Kairox-owned wrappers in `apps/agent-gui/src/components/ui/`; business components should import the Kairox wrappers instead of raw primitive components unless a design spec explicitly approves an exception. Toast notifications are powered by `useToast()` (composable) + `ToastContainer.vue`. Confirmation dialogs use `useConfirm()` (provide/inject) + `ConfirmDialog.vue` with native `<dialog>`.
- **Composable utilities**: `@vueuse/core` (whitelisted via auto-import: `useDark`, `useColorMode`, `useStorage`, `useEventListener`, `tryOnScopeDispose`, `useDebounceFn`, `useThrottleFn`, `useIntervalFn`, `useTimeoutFn`, `useClipboard`, `useFocus`, `whenever`).
- **Auto-imports**: `unplugin-auto-import` + `unplugin-vue-components` are configured in `vite.config.ts` (mirrored in `vitest.config.ts`). The whitelist covers `vue`, `vue-router`, `pinia`, `vue-i18n`, and selected `@vueuse/core` hooks. Project components under `src/components/` are auto-registered in templates. Auto-import only transforms `.vue` files (we keep `dirs: []`); plain `.ts` modules — stores, composables, the router, `locales/index.ts`, `main.ts`, test-utils — still import their `vue`/`pinia`/`vue-i18n`/`@vueuse/core` symbols explicitly. Generated artifacts (`src/auto-imports.d.ts`, `src/components.d.ts`) are gitignored — Vite regenerates them on dev/build.
- **Path alias**: `@/*` resolves to `apps/agent-gui/src/*` (configured in `vite.config.ts` and `tsconfig.json`).
- **Types**: Centralized in `apps/agent-gui/src/types/`. Mirror Rust event types for Tauri IPC.
- **Testing**: Vitest with `vitest/globals` + `@vue/test-utils`. Unit and component tests are colocated with the subject as `*.test.ts` under `src/components/`, `src/stores/`, `src/composables/`, `src/router/`, and related feature folders; split large stores into subfolders such as `src/stores/mcp/*.test.ts`. Test helper `src/test-utils/mount.ts` exposes `mountWithPlugins()` for specs that need pinia + i18n + router; use raw `mount()` for leaf components that do not need those plugins. Use `@pinia/testing`'s `createTestingPinia()` when you want spy-able actions.
- **Style**: oxfmt (formatting) + oxlint (linting) + Stylelint (CSS). See lint-staged config for auto-fix rules.

### Tauri IPC pattern

The GUI follows this pattern:

1. Rust `commands.rs` defines `#[tauri::command]` functions that call `AppFacade` methods
2. `lib.rs` registers all commands, manages `GuiState` (holds `Arc<LocalRuntime<...>>`), and starts the event forwarder
3. Vue frontend calls `invoke("command_name", { args })` via `@tauri-apps/api`
4. Events flow Rust→Vue via `event_forwarder.rs` (using `subscribe_all()`) → `app_handle.emit()` → `useTauriEvents.ts` listener (filters by `currentSessionId`)

### Permission system

The legacy `PermissionMode` enum has been removed end-to-end (`feat(runtime,store,tools,tui,gui)!` in v0.31.0). Tool execution is now driven by two orthogonal policies in `agent-tools/src/policy/`:

- **`ApprovalPolicy`** — when to ask the user for approval (`Never` / `OnRequest` / `Always`)
- **`SandboxPolicy`** — what the sandbox structurally allows (`ReadOnly` / `WorkspaceWrite { network_access, writable_roots }` / `DangerFullAccess`)

`PolicyEngine::decide` takes a `PolicyRisk` and returns a `PolicyDecision` (`Allowed`, `DeniedBySandbox`, or `NeedsApproval { reason }`). `PermissionEngine` is a thin wrapper retained for older call sites. Persisted state lives in `agent-store` migration `0007_approval_sandbox_policy.sql`. See `docs/superpowers/specs/2026-05-26-permission-sandbox-approval-design.md` for the full decision matrix.

Both axes surface in the GUI (`ChatApprovalSelector.vue`, `ChatSandboxSelector.vue`) and the TUI status bar; keep them in sync when changing approval or sandbox behavior.

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
├── package.json            # Bun tooling root (format, lint, prepare)
├── bun.lock
├── cliff.toml              # git-cliff changelog config
├── commitlint.config.js    # Conventional Commits enforcement
├── scripts/
│   ├── release.sh          # Automated release script
│   └── prepare.cjs         # Husky hook setup (worktree-aware)
├── crates/
│   ├── agent-core/         # Domain types, events, facade, IDs, build info, projections
│   ├── agent-runtime/      # LocalRuntime + focused modules: agent_loop, agents,
│   │                       #   dag_executor, event_emitter, execution_runtime (session actors),
│   │                       #   facade_runtime, facade_turn_executor, facade_session_ops,
│   │                       #   mcp_manager, memory_handler, permission, session, task_graph
│   ├── agent-models/       # ModelClient trait + OpenAI / Anthropic / Ollama / Fake adapters
│   ├── agent-tools/        # Tool registry, permission engine, built-in tools
│   │                       #   (shell.exec, fs.read, fs.write, fs.list,
│   │                       #   patch.apply, search.ripgrep, monitor.start/list/stop,
│   │                       #   browser.action, browser.batch, computer.use), MCP adapter
│   ├── agent-mcp/          # MCP client, transports (stdio, sse, streamable_http), lifecycle, health checks,
│   │                       #   protocol types, discovery cache, marketplace catalog
│   ├── agent-skills/       # Native skills system — reusable prompt/tool/workflow capabilities,
│   │                       #   frontmatter parsing, skill registry, config-driven discovery
│   ├── agent-plugins/      # Plugin manifest/inventory parsing and plugin settings helpers
│   ├── agent-memory/       # Memory store, marker/extractor, context assembler (tiktoken), image pruning
│   ├── agent-store/        # SQLite event store, metadata tables, trajectory store
│   ├── agent-config/       # Config loading, profile discovery, MCP server config,
│   │                       #   skills config, `.kairox/` project config discovery
│   ├── agent-eval/         # Headless evaluation harness (`kairox-eval` binary)
│   ├── agent-sdk/          # Embeddable SDK — programmatic runtime API for
│   │                       #   external harnesses, CI/CD, and custom UIs
│   └── agent-tui/          # ratatui TUI app (CLI: --version, --profile, --approval-policy, --sandbox-policy)
├── apps/
│   └── agent-gui/          # Tauri 2 + Vue 3 desktop app
│       ├── src/            # Vue frontend
│       │   ├── App.vue     # thin root: mounts AppLayout, handles workspace bootstrap
│       │   ├── main.ts     # createApp + pinia + router + i18n + bindLocaleToStore
│       │   ├── layouts/AppLayout.vue # ConfirmDialog + ToastContainer + nav + RouterView
│       │   ├── views/      # WorkbenchView, MarketplaceView, SettingsView,
│       │   │               #   SkillsSettingsView, WorkspaceView, setting subviews (lazy)
│       │   ├── router/     # index.ts (createWebHashHistory) + routes.ts
│       │   ├── locales/    # en.json, zh-CN.json, index.ts (i18n instance)
│       │   ├── styles/theme.css      # CSS custom properties (light + dark via html.dark)
│       │   ├── styles/components.css # Shared CSS classes (btn, tag, card, alert, etc.)
│       │   ├── components/ # ChatPanel, TraceTimeline, TaskSteps, TaskNode,
│       │   │               #   PermissionPrompt, chat/* stream items, MemoryBrowser,
│       │   │               #   McpServerManager, McpStatusIndicator, SessionsSidebar,
│       │   │               #   StatusBar, ToastContainer, ConfirmDialog, TraceEntry,
│       │   │               #   ModelSettingsPane, AgentSettingsPane, HooksSettingsPane,
│       │   │               #   PluginSettingsPane,
│       │   │               #   marketplace/* (CatalogList, CatalogCard, CatalogDetail,
│       │   │               #     InstalledList, InstallProgress, RuntimeMissingHint)
│       │   ├── stores/     # session, taskGraph, agents, mcp, memory, catalog, skills, ui
│       │   ├── composables/# useTauriEvents (session-filtered), useTraceStore,
│       │   │               #   useNotifications (delegates to ui store), useToast,
│       │   │               #   useConfirm, useUpdater, useMarketplace
│       │   ├── test-utils/mount.ts # mountWithPlugins helper for vitest
│       │   ├── types/      # TypeScript type definitions (re-exports from generated/)
│       │   │   └── events-helpers.ts  # ExtractPayload, EventPayloadHandlers, matchPayload
│       │   └── generated/  # specta-generated bindings (commands.ts, events.ts)
│       ├── src-tauri/      # Rust Tauri backend
│       │   ├── src/        # commands.rs, app_state.rs, event_forwarder.rs, specta.rs, lib.rs
│       │   ├── Cargo.toml  # version.workspace = true
│       │   └── tauri.conf.json
│       ├── e2e/            # Playwright E2E specs + tauri-mock.js IPC mock
│       ├── package.json
│       └── vite.config.ts
├── docs/
│   ├── dev/                # Local development guide
│   ├── github/             # Discussion templates, labels
│   └── superpowers/        # OpenSpec plans & specs
├── site/                   # VitePress documentation site (deployed via Pages workflow)
│   ├── .vitepress/         # Config, theme overrides, ReleaseBanner/FeedbackBlock components
│   ├── guide/ concepts/ reference/ community/  # English pages
│   └── zh/                 # Chinese parity translations
└── .github/workflows/      # CI, release-build, pages (VitePress deploy), verify-build, dependabot-auto-merge
```

## Commit conventions

Conventional Commits are enforced via commitlint + husky. Use these scopes:

| Scope     | When to use                                         |
| --------- | --------------------------------------------------- |
| `core`    | Changes to `agent-core`                             |
| `runtime` | Changes to `agent-runtime`                          |
| `models`  | Changes to `agent-models`                           |
| `tools`   | Changes to `agent-tools`                            |
| `memory`  | Changes to `agent-memory`                           |
| `store`   | Changes to `agent-store`                            |
| `config`  | Changes to `agent-config`                           |
| `tui`     | Changes to `agent-tui`                              |
| `gui`     | Changes to `apps/agent-gui` (Rust or Vue)           |
| `deps`    | Dependency updates (Cargo or Bun)                   |
| `mcp`     | Changes to `agent-mcp`                              |
| `skills`  | Changes to `agent-skills`                           |
| `plugins` | Changes to `agent-plugins`                          |
| `eval`    | Changes to `agent-eval`                             |
| `sdk`     | Changes to `agent-sdk`                              |
| `docs`    | Documentation, VitePress site, AI instruction files |
| `ci`      | CI/CD workflow changes                              |

Examples:

- `feat(runtime): add scheduler retry policy`
- `fix(gui): handle empty trace state`
- `refactor(memory): extract keyword scoring into separate module`
- `chore(deps): bump reqwest to 0.12`
- `feat(mcp): add SSE transport support`

## Branch conventions

For any non-trivial development task (new features, bug fixes, refactors), **always create a feature branch** instead of committing directly to `main`. Use Conventional Commit prefixes as branch names:

| Branch prefix | When to use                                | Example                            |
| ------------- | ------------------------------------------ | ---------------------------------- |
| `feat/`       | New features or enhancements               | `feat/gui-interaction-polish`      |
| `fix/`        | Bug fixes                                  | `fix/streaming-stuck`              |
| `refactor/`   | Code restructuring without behavior change | `refactor/extract-context-builder` |
| `test/`       | Adding or improving tests                  | `test/runtime-integration`         |
| `docs/`       | Documentation changes                      | `docs/api-reference`               |
| `chore/`      | Tooling, CI, dependencies                  | `chore/bump-deps`                  |
| `ci/`         | CI/CD workflow changes                     | `ci/parallel-jobs`                 |

**Workflow:**

1. Create a branch: `git checkout -b feat/my-feature main`
2. Develop and commit with Conventional Commit messages
3. Push the branch: `git push origin feat/my-feature`
4. Open a pull request for review
5. Merge via PR — do not push directly to `main`

**Quick branch creation with just:**

```bash
just worktree feat/my-feature   # creates .worktrees/feat-my-feature and runs bun install
```

Small fixes (typos, trivial one-liners) may be committed directly to `main`, but anything touching more than one file or requiring review should use a branch.

## Git worktrees

This project uses git worktrees for isolated branch development. Worktrees live under the project-local, ignored `.worktrees/` directory. Keep the git branch name in Conventional Commit form (`feat/my-feature`, `fix/streaming-stuck`, etc.) and use a sanitized directory name where path separators are replaced with `-`:

```bash
branch=feat/my-feature
worktree=.worktrees/feat-my-feature

git check-ignore -q .worktrees
git worktree add "$worktree" -b "$branch" main
cd "$worktree"
bun install    # triggers prepare.cjs which links husky hooks
```

Prefer `just worktree <branch>` for new worktrees. The recipe creates `.worktrees/<sanitized-branch-name>`, starts the branch from `main`, and runs `bun install`.

The `.worktrees/` directory must remain ignored so nested worktree contents are never committed. The `prepare.cjs` script detects worktrees and creates a symlink from `GIT_DIR/.husky` to the worktree's `.husky` directory so that pre-commit and commit-msg hooks fire correctly.

## Local verification

Run before opening a PR or pushing to main:

```bash
bun run format:check
bun run lint
cargo test --workspace --all-targets
```

Pre-commit hooks (husky + lint-staged) automatically run on staged files:

- `*.rs` → `cargo fmt --all`
- `*.{json,md}` → `oxfmt --write`
- `apps/agent-gui/**/*.{ts,tsx,js,jsx,vue}` → `oxfmt --write` + `oxlint --fix`
- `apps/agent-gui/src/**/*.{vue,css,scss,sass,less}` → `oxfmt --write` + `stylelint --fix`

## Version bumping

When bumping the version for a release, edit these files (all must stay in sync):

1. **`Cargo.toml`** — `workspace.package.version`
2. **`Cargo.lock`** — run `cargo generate-lockfile` to update all crate versions
3. **`apps/agent-gui/package.json`** — `"version"` field
4. **`apps/agent-gui/src-tauri/tauri.conf.json`** — `"version"` field
5. **`package.json`** (root) — `"version"` field

Do NOT edit `version` in individual crate `Cargo.toml` files — they inherit from the workspace.

> **⚠️ AI assistant reminder**: Bumping the version number alone is NOT sufficient. Every version bump MUST be followed by the full release flow: update CHANGELOG (`git cliff`), commit changelog, create the git tag, and push both the branch and tag to remote. Missing any of these steps will cause release artifacts (installers, CHANGELOG, GitHub Release) to be incomplete or missing. If you only bump the version without completing the release flow, the version will not have a corresponding release.

## Release flow

Use `scripts/release.sh <version>` to publish a release:

```bash
scripts/release.sh 0.7.0
```

The script runs checks, verifies the GUI build, generates `CHANGELOG.md` with git-cliff, commits it, creates the tag, and pushes. Supports `--dry-run`, `--skip-checks`, `--skip-build`, and `--prerelease` options.

### Manual release steps (if not using the script)

1. Bump version in all config files (see above)
2. Commit the version bump: `git commit -m "chore(release): bump version to X.Y.Z"`
3. Run `git cliff --tag vX.Y.Z -o CHANGELOG.md`
4. Commit the changelog: `git commit -m "chore(release): update CHANGELOG for vX.Y.Z"`
5. Open a release PR, wait for the `ci-success` gate to pass, and merge it to `main`
6. Create and push the tag from the merged `main` commit: `git checkout main && git pull --ff-only origin main && git tag -fa vX.Y.Z -m "vX.Y.Z" && git push origin vX.Y.Z -f`

**Always commit an updated `CHANGELOG.md` before merging and pushing the release tag.** The tag should point to a `main` commit that includes the changelog update.

### How git-cliff works

- `cliff.toml` at the repo root configures the changelog format and commit grouping
- Commits are grouped into Features, Bug Fixes, Performance, Documentation, Testing, Refactor, Styling, CI, Dependencies, and Miscellaneous
- `chore(release):` commits are automatically excluded from the changelog
- GitHub Actions also runs git-cliff to generate Release Notes on the GitHub Release page

## CI

- **CI** (`ci.yml`) runs on push to `main` and on pull requests: format check, lint, cargo test, TUI build, GUI web build, type-sync, Playwright E2E, tauri-pilot desktop E2E, coverage tiers (Rust + Web), and live model smoke tests
- **Release Build** (`release-build.yml`) runs on `v*` tags: publishes release notes via git-cliff, builds TUI binaries for all platforms, builds Tauri desktop bundles for all platforms
- **Pages** (`pages.yml`) deploys the VitePress site under `site/` to GitHub Pages on push to `main` matching `site/**`, `README.md`, `bun.lock`, `package.json`, `docs/assets/**`, or `.github/workflows/pages.yml`. Refetches the latest GitHub Release at build time to populate `site/.vitepress/cache/release.json` (banner only refreshes on the next Pages run after a tag is released). Tag pushes do NOT trigger Pages.
- **Verify Build** (`verify-build.yml`) sanity-builds artifacts on PR
- **CodeQL** (`codeql.yml`) static analysis for security findings
- **Dependabot Auto Merge** automatically merges passing Dependabot PRs for Bun, Cargo, and GitHub Actions dependencies

## Coverage gates

Coverage gates are organised by **risk tier**, not by codebase area. Stricter tiers fail first; relaxed tiers act as a workspace-wide anti-truncation backstop. Two CI jobs enforce the gates:

- `coverage-rust` — runs `bun run coverage:rust` (cargo-llvm-cov + grcov → LCOV → `scripts/check-rust-coverage.mjs`).
- `coverage-web` — runs `bun run coverage:web` (Vitest V8 with thresholds in `apps/agent-gui/vitest.config.ts`).

### Rust tiers (`scripts/check-rust-coverage.mjs`)

Floors below are the gates enforced today, calibrated against CI's actual LCOV output. Branch coverage is **not** gated: `--branch` triggers a SIGSEGV in `llvm::coverage::CoverageMapping::getInstantiationGroups` when `llvm-cov` processes async test binaries on this workspace (upstream LLVM bug [llvm/llvm-project#189169](https://github.com/llvm/llvm-project/issues/189169)), and the workaround is to drop `--branch` from `cargo +nightly llvm-cov` and `grcov` invocations. functions/lines stay; per-tier numbers are conservative (≈ `floor(actual − 1)`) and will tighten as the pipeline surfaces more files.

| Tier                     | Path patterns                                                                                                                            | functions | lines |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------- | --------- | ----- |
| **T1 Critical**          | `agent-tools/src/{permission,registry}.rs`, `agent-store/src/`, `agent-core/src/`, `agent-config/src/`                                   | 37        | 84    |
| **T2 High runtime**      | `agent-runtime/src/`, `agent-memory/src/`, `agent-models/src/`, `agent-mcp/src/`                                                         | 27        | 78    |
| **T2 Tauri IPC**         | `apps/agent-gui/src-tauri/src/{lib,app_state,event_forwarder,commands}.rs`, `apps/agent-gui/src-tauri/src/commands/` (excl. `specta.rs`) | 2         | 18    |
| **T3 Adapters & skills** | `agent-tools/src/` (excl. T1), `agent-skills/src/`, `agent-plugins/src/`                                                                 | 72        | 92    |
| **T4 Floor**             | `agent-tui/src/`, `agent-eval/src/`                                                                                                      | 37        | 62    |
| Workspace overall        | `crates/`, `apps/agent-gui/src-tauri/src/`                                                                                               | 30        | 70    |

Each group also enforces a `minFiles` floor to catch report truncation. Groups can opt into `allowPartial: true` to warn rather than fail when no files match (no group currently uses it after #509 — every workspace crate now appears in LCOV).

**Tier targets** (what the floors should approach as tests are added): T1 70/85 · T2 High 55/80 · T2 Tauri 50/70 · T3 80/95 · T4 50/65 · Workspace 50/75 (functions/lines).

### Web tiers (`apps/agent-gui/vitest.config.ts`)

| Tier               | Glob                           | statements | branches | functions | lines |
| ------------------ | ------------------------------ | ---------- | -------- | --------- | ----- |
| Global             | aggregate                      | 80         | 72       | 76        | 80    |
| **T1 Utils**       | `src/utils/**/*.ts`            | 92         | 90       | 95        | 92    |
| **T2 Stores**      | `src/stores/**/*.ts`           | 80         | 68       | 80        | 82    |
| **T2 Composables** | `src/composables/**/*.ts`      | 74         | 60       | 78        | 74    |
| **T3 Components**  | `src/components/**/*.{ts,vue}` | 78         | 72       | 74        | 78    |
| **T3 Views**       | `src/views/**/*.vue`           | 90         | 82       | 78        | 90    |

### Local calibration

```bash
bun run coverage:rust   # macOS may hit aws-lc-sys + Apple clang panic; rely on CI
bun run coverage:web    # works on macOS without extra setup
```

`bun run coverage:rust` ends with `Rust coverage thresholds satisfied.` on success or prints a list of failing groups + metric deltas.

### Adjusting thresholds

When real coverage tracks above a threshold by ≥3pp on `origin/main` for several PRs in a row, raise the threshold in the same PR that added the new tests, by editing the relevant entry in `scripts/check-rust-coverage.mjs` (Rust) or `apps/agent-gui/vitest.config.ts` (Web). When a threshold blocks a legitimate PR, prefer adding tests over lowering the threshold; only fall back to a one-line `metric: floor(actual − 1)` exemption when the gap is < 5pp and the gap is documented in the PR body with a follow-up plan.

`KAIROX_COVERAGE_ALLOW_PARTIAL=1` only suppresses "no files matched" warnings (useful when running `coverage:rust` against a partial workspace). It does **not** bypass threshold checks.

## AI coding guidelines

### tauri-specta for type generation

[tauri-specta](https://github.com/specta-rs/tauri-specta) auto-generates TypeScript types from Rust definitions. TypeScript bindings are generated into two files:

| File                                       | What it covers                                                                                                                       |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| `apps/agent-gui/src/generated/commands.ts` | `#[tauri::command]` function signatures and return types                                                                             |
| `apps/agent-gui/src/generated/events.ts`   | `EventPayload`, `DomainEvent`, `AgentRole`, `TaskState`, `TaskSnapshot`, `TaskGraphSnapshot`, `MemoryScope`, `PrivacyClassification` |

**When to regenerate**: after adding/modifying/removing any `#[tauri::command]` function, its parameter/return types, or any `EventPayload` variant / domain type used in events:

```bash
just gen-types
```

**How it works for commands**: add `#[specta::specta]` to the command function, register it in `collect_commands![]` (in `src/specta.rs`) and `generate_handler![]` (in `src/lib.rs`), then run `just gen-types`.

**How it works for events**: domain types in `agent-core` and `agent-memory` have `#[cfg_attr(feature = "specta", derive(specta::Type))]` attributes. The `agent-gui-tauri` crate enables the `specta` feature and registers these types in `src/specta.rs`. The `export-events` binary generates `events.ts`. TypeScript consumers use discriminated union narrowing (no `as` casts needed).

**Type sync check**: `just check-types` runs `gen-types` and verifies no uncommitted changes in `apps/agent-gui/src/generated/`. The `type-sync` CI job enforces this.

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

### Rust test organization

- Put focused tests for private helpers in sibling files named `src/<module>_tests.rs`, then include them from the owning module with `#[cfg(test)]` and `#[path = "<module>_tests.rs"] mod tests;`. This keeps production files readable while preserving private-item access.
- Put crate integration tests in `crates/<crate>/tests/*.rs` when the test should use the crate as a consumer would. Use `tests/<suite>/main.rs` for large suites and keep reusable builders, fake runtimes, and assertions in a suite-local `support` module or `tests/support/`.
- Keep fixtures under `crates/<crate>/tests/fixtures/` when the test consumes TOML, JSONL, plugin manifests, MCP server fixtures, or sample workspaces.
- Runtime tests should prefer `FakeModelClient` for deterministic text streams and `ToolCallingModel` for tool-call paths. Persistence tests should use in-memory `SqliteEventStore` / `SqliteMemoryStore` unless the behavior depends on filesystem paths.
- Feature-gated or external-service tests belong in their own integration target, like `live_model_tests`, and must self-skip cleanly when required credentials are absent.

### When modifying the GUI

- Vue components go in `apps/agent-gui/src/components/`. Use native HTML elements with CSS classes from `src/styles/components.css` (`.btn`, `.card`, `.tag`, `.alert`, etc.) and CSS custom properties from `src/styles/theme.css`. For toasts use `useToast()`; for confirmation dialogs use `useConfirm()`.
- Pinia stores live in `apps/agent-gui/src/stores/` and use the setup-store form (`defineStore('name', () => ({ /* state, getters, actions */ }))`). Cross-store dependencies should be resolved lazily inside actions (e.g. `const session = useSessionStore()` _inside_ the function body, not at module top level).
- Composables go in `apps/agent-gui/src/composables/`. Use `tryOnScopeDispose` (auto-imported from `@vueuse/core` inside `.vue` files; explicitly imported in plain `.ts`) for cleanup of `listen()` subscriptions.
- Routes go in `apps/agent-gui/src/router/routes.ts`. Use `useRoute`/`useRouter` (auto-imported in templates) inside components.
- i18n: add new common-copy keys to BOTH `apps/agent-gui/src/locales/en.json` AND `apps/agent-gui/src/locales/zh-CN.json`. Reach for `t("common.send")` in templates. Per-feature strings can stay inline.
- Theme: CSS custom properties are defined in `apps/agent-gui/src/styles/theme.css` (light defaults in `:root`, dark overrides in `html.dark`). Toggle dark mode via `useUiStore().setTheme('dark')`. Add new design tokens as `--app-*` variables.
- TypeScript types go in `apps/agent-gui/src/types/`.
- Auto-generated event types are in `apps/agent-gui/src/generated/events.ts` — **never edit this file manually**, run `just gen-types` instead.
- Event helper types (`ExtractPayload`, `EventPayloadHandlers`, `matchPayload`) are in `apps/agent-gui/src/types/events-helpers.ts`.
- Always update the corresponding Rust `#[tauri::command]` in `commands.rs` if the IPC surface changes.
- Use `useTauriEvents.ts` for real-time Rust→Vue event streaming.
- Use TypeScript discriminated union narrowing (not `as` casts) when handling `EventPayload` variants.
- For tests, colocate Vitest specs with the thing they exercise: `ComponentName.test.ts` beside components, `useThing.test.ts` beside composables, and `storeName.test.ts` or `stores/<domain>/*.test.ts` beside stores. Prefer raw `mount` for pure leaf components; use `mountWithPlugins` from `src/test-utils/mount.ts` only when the component needs pinia, i18n, router, or an initialized route.

### E2E testing with Playwright

The GUI frontend has comprehensive E2E tests using Playwright that run against the Vite dev server with a browser-side Tauri IPC mock (`apps/agent-gui/e2e/tauri-mock.js`). This mock replaces `@tauri-apps/api` calls so the full Vue frontend can be tested without a real Tauri runtime.

**When to update the mock**: if you add or change any `#[tauri::command]` function signature or its parameter/return types, update the domain fragment under `apps/agent-gui/e2e/fixtures/tauri-mock/` and register it through the mock registry. The top-level `apps/agent-gui/e2e/tauri-mock.js` installs the bridge; specs should use `installTauriMock(page)` from `e2e/helpers/tauriMock.ts`. If you add new event types that the frontend listens to, add the corresponding event emission in the mock fixture.

**Running E2E tests**:

```bash
just test-e2e              # headless CI mode
just test-e2e-headed       # headed mode for debugging
just test-e2e-ui           # Playwright UI mode
```

**Test structure**: Playwright specs live under `apps/agent-gui/e2e/*.spec.ts` and should cover user-visible workflows rather than isolated store internals. Shared page helpers live under `apps/agent-gui/e2e/helpers/`, while mock state and command handlers live under `apps/agent-gui/e2e/fixtures/tauri-mock/`.

### TUI, runtime, and desktop integration tests

TUI app logic tests (`crates/agent-tui/tests/app_logic_*.rs`) use `FakeModelClient` + in-memory `SqliteEventStore` to test the `LocalRuntime` facade without a real terminal. Full-stack runtime tests (`crates/agent-runtime/tests/full_stack/main.rs`) exercise the complete pipeline including tool calling, permission decisions, and persistence. Other large runtime suites use the same `tests/<suite>/main.rs` shape with suite-local `support`.

```bash
just test-tui              # TUI app logic integration tests
just test-fullstack        # full-stack runtime tests (currently 13 in full_stack.rs)
just test-mcp              # MCP integration tests across mcp/tools/config/runtime
just test-all              # all test layers: unit + integration + fullstack + GUI Vitest
```

> Additional integration tests live under `crates/agent-runtime/tests/`, including `agent_loop/`, `session_lifecycle/`, `task_graph_integration.rs`, `memory_protocol.rs`, `mcp_integration.rs`, `refactor_baseline.rs`, and `fake_session.rs`. They are all picked up by `cargo test --workspace`.

### Tauri pilot E2E (full-stack desktop tests)

Beyond the Playwright tests (which mock the Tauri IPC boundary), Kairox also runs a **real desktop E2E** stack on top of [`tauri-plugin-pilot`](https://github.com/mpiton/tauri-pilot). The plugin exposes a Unix-socket JSON-RPC 2.0 interface inside a running Tauri app, and the `tauri-pilot` CLI drives TOML-defined scenarios against it.

**Double feature gating** — to keep release binaries safe, the pilot plugin is only registered when **both** conditions hold:

1. The build is a debug profile (`debug_assertions` is set), and
2. The cargo feature `pilot` is explicitly enabled (`apps/agent-gui/src-tauri/Cargo.toml`).

The `apps/agent-gui/src-tauri/build.rs` mirrors this: it only loads the dedicated capability file `apps/agent-gui/src-tauri/capabilities/pilot.json` (which grants `pilot:default`) when both gates are open. The default capability file does not reference the pilot permission, so a release build neither links the plugin nor advertises the capability.

**Scenarios** live under `apps/agent-gui/e2e-pilot/*.toml`:

- Bootstrap smoke scenarios: `app-bootstrap.toml`, `chat-flow.toml`, `session-lifecycle.toml`
- Audit scenarios: `audit-bootstrap.toml`, `audit-chat.toml`, `audit-sessions.toml`, `audit-marketplace-memory.toml`, `audit-mcp.toml`
- Each scenario should prefer stable `data-test` selectors and avoid arbitrary sleeps unless it documents a known driver/runtime race.

**Running locally** (`just test-pilot`):

```bash
just test-pilot     # builds the Tauri debug binary with --features pilot, then runs the scenarios
```

The recipe invokes `bun --filter agent-gui tauri build --debug --no-bundle --features pilot` followed by `scripts/run-pilot-tests.sh`. The script writes per-scenario JUnit XML to `pilot-results/<name>.xml` and dumps screenshots/logs into `tauri-pilot-failures/` on failure (both directories are gitignored). On Linux you usually need `xvfb-run -a just test-pilot`; on macOS the Tauri window appears briefly during the run.

**Prerequisite**: `tauri-pilot-cli` must be on `PATH`. Install with:

```bash
cargo install --git https://github.com/mpiton/tauri-pilot tauri-pilot-cli
```

**CI** runs the matching `tauri-pilot-e2e` job in `.github/workflows/ci.yml` against the `ubuntu-latest` (with `xvfb-run`) and `macos-latest` runners; per-scenario JUnit XML is uploaded as a `pilot-results-${{ matrix.os }}` artifact, and on failure the `tauri-pilot-failures-${{ matrix.os }}` artifact preserves screenshots from the local `tauri-pilot-failures/` directory.

**When to update the scenarios**: any time you change UI markers (`data-test='...'`) referenced by the TOML scenarios, add a new bootstrap-critical view, or adjust the chat-send flow.

### Live model integration tests

To guard against silent regressions in the OpenAI-compatible model client, `crates/agent-runtime/tests/live_model_tests.rs` exercises a real model API (GitHub Models, `openai/gpt-4o-mini`) end-to-end. The test is gated behind the `live-model-tests` cargo feature so the regular `cargo test --workspace` stays hermetic.

**Behavior without a token**: the test reads `GITHUB_TOKEN` via `std::env::var`. When it is absent the test prints a skip notice and returns early — it never panics. This means `just test-live` is safe to run locally without configuring credentials.

**Profile + fixture**:

- Fixture: `fixtures/test-profiles/github-models.toml` (profile `github-gpt4o-mini`, `provider = "openai_compatible"`, `base_url = "https://models.github.ai/inference"`, `api_key_env = "GITHUB_TOKEN"`).
- The test loads it via `agent_config::loader::{load_from_str, resolve_api_keys, validate}` + `build_router`, then sends a one-shot prompt and asserts the stream emits a `Completed` event with non-empty content. Both the stream open and stream drain are wrapped in a 60s `tokio::time::timeout`.

**Running locally** (`just test-live`):

```bash
just test-live      # cargo test -p agent-runtime --features live-model-tests --test live_model_tests -- --nocapture
```

The `-- --nocapture` flag is what surfaces the skip notice on stderr when `GITHUB_TOKEN` is absent; running the bare `cargo test` form will still pass but the skip message stays hidden behind cargo's default output capture.

**CI** runs the matching `live-model-tests` job with `permissions: { contents: read, models: read }` so the auto-injected `GITHUB_TOKEN` (with the GitHub Models scope) can call the inference API. The job times out after 15 minutes.

**When to update the fixture**: only when GitHub Models retires the chosen model or moves the inference endpoint. The model is intentionally on the Low tier (15 RPM / 150 RPD) for CI safety.

### Common pitfalls

- **Don't add crate-level `version`**: all crates use `version.workspace = true`
- **Don't skip `cargo clippy`**: CI denies warnings
- **Don't use `npm`, `pnpm`, or `yarn`**: this project uses Bun for JavaScript package management
- **Don't forget `bun install` after creating a worktree**: husky hooks won't fire otherwise
- **Don't hardcode API keys**: use `agent-config`'s `api_key_env` to reference environment variables
- **Don't forget to run `just gen-types`** when changing Rust event/domain types — the TypeScript types are auto-generated, not manually maintained
- **Don't forget to register new Tauri commands in both `generate_handler!` (for invocation) and `collect_commands!` (for specta type generation)**; missing either one causes runtime or type-gen failures
- **Don't import what's auto-imported in `.vue` files**: `vue`, `vue-router`, `pinia`, `vue-i18n`, and the whitelisted `@vueuse/core` hooks listed in `vite.config.ts` are globals inside SFCs. Re-importing them in a `.vue` file creates a "duplicate import" warning at lint time. The exception is when shadowing or aliasing — use explicit imports then.
- **Plain `.ts` modules still need explicit imports**: auto-import only transforms `.vue` files (we keep `dirs: []`). Stores, composables, the router, `locales/index.ts`, `main.ts`, and test-utils MUST keep explicit `import { defineStore } from "pinia"` / `import { ref, computed } from "vue"` / `import { createI18n } from "vue-i18n"` etc. Otherwise the browser hits `Uncaught ReferenceError: createI18n is not defined` at module load and the app never mounts.
- **Don't commit `apps/agent-gui/src/auto-imports.d.ts` or `apps/agent-gui/src/components.d.ts`** — they are regenerated on every Vite dev/build and are listed in `.gitignore`.
- **Don't use `useConfirm()` outside a component wrapped by `<ConfirmDialog>`** — `inject()` will throw. The provider lives in `AppLayout.vue`. For toasts, `useToast()` works anywhere Pinia is active.
- **Don't navigate via `view = ref('workbench')` patterns**: vue-router is the source of truth. Use `router.push({ name: 'workbench', params: { sessionId } })` and read state via `useRoute()`.
- **Don't forget `xvfb-run -a` when running `just test-pilot` on Linux** — `tauri build --debug` produces a real GUI binary that requires a display. macOS and Windows runners use the native window server.
- **Don't assume `just test-live` without `GITHUB_TOKEN` is broken** — the test self-skips with an `eprintln!` notice and exits 0 by design, so the recipe stays safe to run locally without credentials.

## Privacy defaults

The initial runtime stores event envelopes and full fake-session content in SQLite during tests. Production configuration must default to `minimal_trace` when a real model or shell tool is configured.

## Quick command reference (`just`)

A `justfile` is provided for common tasks. Install with `cargo install just` or `brew install just`.

| Command                   | Description                                                                            |
| ------------------------- | -------------------------------------------------------------------------------------- |
| `just check`              | Full CI gate: format check + lint + test                                               |
| `just fmt-check`          | Check formatting (Rust + web)                                                          |
| `just lint`               | Run clippy + oxlint + stylelint                                                        |
| `just test`               | Run all Rust tests                                                                     |
| `just test-gui`           | Run GUI (Vitest) tests                                                                 |
| `just fmt`                | Auto-format all code                                                                   |
| `just tui`                | Run the TUI app                                                                        |
| `just gui-dev`            | Run GUI dev server (Vite)                                                              |
| `just tauri-dev`          | Run Tauri desktop app in dev mode                                                      |
| `just bump-version X.Y.Z` | Bump version in all config files                                                       |
| `just check-types`        | Verify generated TypeScript types are in sync with Rust                                |
| `just test-e2e`           | Run GUI frontend E2E tests with Playwright                                             |
| `just test-e2e-headed`    | Run E2E tests in headed mode for debugging                                             |
| `just test-e2e-ui`        | Run E2E tests with Playwright UI mode                                                  |
| `just test-tui`           | Run TUI app logic integration tests                                                    |
| `just test-fullstack`     | Run full-stack runtime integration tests                                               |
| `just test-all`           | Run all test layers: unit + integration + E2E + TUI                                    |
| `just test-mcp`           | Run MCP integration tests                                                              |
| `just test-live`          | Run the live GitHub Models integration test (self-skips without `GITHUB_TOKEN`)        |
| `just test-pilot`         | Build the Tauri debug binary with `--features pilot` and run the tauri-pilot scenarios |
| `just worktree <name>`    | Create a git worktree with bun install                                                 |

## Common workflow recipes

### Adding a new event type

1. **Add the variant** to `EventPayload` in `crates/agent-core/src/events.rs` (along with any new structs in `task_types.rs` if task-related)
2. **Add the match arm** in `EventPayload::event_type()` (same file)
3. **If adding types in agent-core or agent-memory**, ensure they have `#[cfg_attr(feature = "specta", derive(specta::Type))]` and are registered in `apps/agent-gui/src-tauri/src/specta.rs`
4. **Run `just gen-types`** to regenerate TypeScript bindings (both `commands.ts` and `events.ts`)
5. **Emit the event** from the appropriate place in `agent-runtime` (e.g., `facade_runtime.rs`, `event_emitter.rs`, `dag_executor.rs`, or `mcp_manager.rs`)
6. **Handle the event** in the UI — TypeScript will error on non-exhaustive `switch` statements if a variant is missing:
   - TUI: update the relevant component in `crates/agent-tui/src/components/`
   - GUI: update `useTraceStore.ts` or the relevant Pinia store/composable

### Adding a new tool

1. **Implement the `Tool` trait** in a new module under `crates/agent-tools/src/` (e.g., `my_tool.rs`)
2. **Register the tool** in `crates/agent-tools/src/registry.rs` via `BuiltinProvider`
3. **Define risk level** in `crates/agent-tools/src/permission.rs` using `ToolRisk`
4. **Add tests** in `crates/agent-tools/src/my_tool.rs` under `#[cfg(test)]`
5. **Wire into runtime** in `crates/agent-runtime/src/facade_runtime.rs` (and `permission.rs` if it introduces new effects) — register the tool in the `ToolRegistry`
6. **Update permission UI** if the tool has a new `ToolEffect` variant (TUI: `permission_modal.rs`, GUI: `PermissionPrompt.vue` / chat-stream permission items)

### Adding a new model provider

1. **Implement `ModelClient` trait** in a new module under `crates/agent-models/src/` (e.g., `my_provider.rs`)
2. **Add model metadata** so routing and context assembly know the provider's context window and token budget defaults
3. **Add a config struct** (e.g., `MyProviderConfig`) with `base_url`, `api_key_env`, etc.
4. **Register in `ModelRouter`** via `crates/agent-models/src/router.rs`
5. **Add profile entry** in `crates/agent-config/src/builder.rs` to map provider string → client constructor
6. **Update `ProfileDef` docs** in `crates/agent-config/src/lib.rs` and `kairox.toml.example`
7. **Add tests** using the existing `FakeModelClient` pattern as a reference; include context-window and model-switching behavior when applicable

### Adding a new MCP server integration

1. **Define server config** in `crates/agent-config/src/` — add fields to parse `[mcp_servers.XXX]` from `kairox.toml`
2. **Add transport** if needed under `crates/agent-mcp/src/transport/` (existing modules: `stdio.rs`, `sse/`, `streamable_http.rs`) — implement the `Transport` trait
3. **Test the server** by adding a fixture or integration test in `crates/agent-mcp/tests/` or `crates/agent-runtime/tests/mcp_integration.rs`
4. **Wire into runtime** via `McpServerManager` in `crates/agent-runtime/src/mcp_manager.rs`
5. **Update permission UI** — TUI: `permission_modal.rs`, GUI: add MCP trust handling in `PermissionPrompt.vue`
6. **Add E2E test** in `apps/agent-gui/e2e/` — update `tauri-mock.js` with new MCP commands
7. **Update config example** in `kairox.toml.example` with the new server configuration

### Adding a new GUI component

1. **Create the Vue SFC** in `apps/agent-gui/src/components/` with `<script setup lang="ts">`
2. **If it needs Tauri IPC**: add a `#[tauri::command]` in `apps/agent-gui/src-tauri/src/commands.rs`, register it in `lib.rs`
3. **If it needs reactive state**: create a Pinia store in `apps/agent-gui/src/stores/` or a composable in `composables/`
4. **If it handles events**: use `useTauriEvents.ts` to listen for `DomainEvent` payloads
5. **Add types** to `apps/agent-gui/src/types/` as needed
6. **Import and use** the component in `App.vue` or the relevant parent component

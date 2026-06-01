---
title: Roadmap
description: Curated highlights of what is shipped, what is in flight, and what is on the horizon.
outline: [2, 3]
---

# Roadmap

::: tip Source of truth
The canonical roadmap is [`ROADMAP.md`](https://github.com/Z-Only/kairox/blob/main/ROADMAP.md) in the repository. This page curates the highlights so you can scan the direction without reading 100+ entries. If this page disagrees with `ROADMAP.md`, the repository file wins.
:::

Kairox is in active pre-1.0 development. The roadmap is organized by horizon: what we already ship, what we are working on now, and the shape of the longer-term bet.

## What ships today (v0.35.x)

The current release covers the foundation across runtime, UIs, MCP, skills, and packaging.

### Runtime and core

- Shared Rust workspace with the `AppFacade` trait as the single seam between UIs and the runtime.
- Event-sourced state with `SqliteEventStore`; sessions persist across restarts.
- Monitor domain events for long-running background work, with lifecycle cleanup when a session ends.
- Agent loop with per-model context windows, budget-driven prompt assembly, manual and automatic compaction, and busy-state guards.
- Mid-session model switching with profile preservation; reasoning effort selection where supported.
- Phase 2 DAG execution with `AgentStrategy` for multi-agent orchestration (planner / worker / reviewer).
- Race-free auto-compaction at turn end (PRs #531–#534).

### Tools, permissions, MCP, LSP/DAP

- Built-in tools: `shell.exec`, `fs.read`, `fs.write`, `fs.list`, `patch.apply`, `search.ripgrep`, plus monitor registry tools (`monitor.start`, `monitor.list`, `monitor.stop`).
- Native **LSP and DAP integration** (`agent-lsp` crate): LSP client for code intelligence (go-to-definition, references, completions, diagnostics) and DAP client for debugger integration; server lifecycle management and dynamic tool registration via `LspToolProvider` / `DapToolProvider`.
- Orthogonal Approval × Sandbox policy engine: `ApprovalPolicy` (`Never` / `OnRequest` / `Always`) gates _when_ the user is asked; `SandboxPolicy` (`ReadOnly` / `WorkspaceWrite` / `DangerFullAccess`) gates _what_ the runtime structurally allows. The legacy single-axis `PermissionMode` enum was removed end-to-end in v0.31.0 (PRs #517, #520).
- MCP client with stdio, SSE, and Streamable HTTP transports, lifecycle management (`McpServer{Starting,Ready,Stopped,Failed}`), and server diagnostic summaries.
- MCP marketplace with built-in catalog plus remote sources; one-click install with runtime requirement hints.
- MCP connectivity actions in the GUI.

### Memory and context

- `<memory>` marker protocol with session / user / workspace scopes and approval semantics.
- Memory browser in the GUI; deletion via TUI trace panel.
- Tiktoken-based context budgeting with auto-compaction at a configurable threshold.

### UIs

- **TUI** built on ratatui: three-pane layout, streaming chat, monitor stream items, trace panel, permission overlay, command palette, settings/marketplace overlays, monitor list/stop commands, model overlay with context-window details, monitor overlay for listing and stopping monitors, remote skill search and install via skills overlay, and trace export / config refresh commands.
- **GUI** built on Tauri 2 + Vue 3: persistent sessions, task graph, searchable trace timeline, memory browser, monitor chat stream rendering with trace-store handling, inline permission flow, per-session `ApprovalPolicy` and `SandboxPolicy` selectors, resizable workbench sidebars, project workspaces, settings tabs for models / agents / MCP / skills / plugins / hooks / instructions, and Tauri IPC controls for monitor list/stop.
- Tauri 2 auto-update wired to GitHub Releases.

### Extensibility

- Native **skills** with workspace / user / session scopes; SkillHub install support.
- **Plugins** with manifests bundling skills, tools, hooks, and MCP servers; plugin-namespaced skill discovery; permission hints, compatibility metadata, and trust metadata for marketplace display and future install policy.
- Configurable agent overrides per role (model, `ApprovalPolicy`, `SandboxPolicy`, skills, tool allowlists, reasoning effort).

### Quality and CI

- Parallel CI with aggregation `ci-success` job; type-sync gate via tauri-specta; clippy, oxlint, stylelint, oxfmt.
- Playwright frontend E2E with browser-side IPC mock.
- `tauri-pilot` real desktop E2E scenarios.
- Live GitHub Models smoke test gated by `GITHUB_TOKEN`.
- `kairox-eval` headless harness with deterministic smoke, tool-call, compaction, and tag-filter scenarios; list mode; fail-fast runs; JSONL, summary JSON, and combined report JSON output; expectations cover required/forbidden event types, tool invocation limits, tool failure limits, elapsed time budgets, and context-token budgets.
- Per-crate coverage gates for Rust and Vue.

For the full shipped list with PR links, scroll the **Near term** section of [`ROADMAP.md`](https://github.com/Z-Only/kairox/blob/main/ROADMAP.md).

## What is in flight (mid term)

- Broader model provider coverage and richer profile policies.
- Continued MCP ecosystem expansion beyond Streamable HTTP: richer discovery and broader server integration polish.
- Signed plugin manifests, remote plugin registries, install/upgrade UX, and plugin sandboxing aligned with the `ApprovalPolicy × SandboxPolicy` engine.
- Configurable specialist subagent roles beyond planner / worker / reviewer, with per-agent context windows, tool allowlists, and reasoning effort.
- Background and long-running parallel agents with cancellation, resumable sessions, and durable status surfacing in TUI/GUI.
- Broader non-interactive and batch-run workflows built on `kairox-eval`.
- User-extensible slash commands, output styles, and statusline customization in TUI and GUI.
- Observability and replay tooling beyond structured trace export: event replay over `EventStore` and redacted diagnostics bundles.
- Continued runtime modularization beyond `SessionActor`.

## Long-term direction

The longer-term bet is a mature local-first AI agent workbench with:

- A strong **skills ecosystem** for composable workflows, reusable instructions, and capability discovery.
- A strong **plugin ecosystem** built on MCP + the tool registry + signed manifests + marketplace governance.
- Rich multi-agent collaboration: delegation, arbitration, specialist teams, shared memory, auditable handoffs.
- Cross-platform desktop distribution polish and auto-update support.
- A telemetry-free privacy story with `minimal_trace` defaults in production.

## How to influence the roadmap

- **Use case feedback** — open a [discussion](https://github.com/Z-Only/kairox/discussions) describing what you are trying to build and where Kairox falls short.
- **Concrete proposals** — open a discussion or an issue with a design sketch. We prefer specs in `docs/superpowers/specs/` for non-trivial work; see [Contributing](./contributing).
- **Pull requests** — most shipped items started as community-authored PRs. The contribution flow is in [Contributing](./contributing).

## Versioning and what counts as "shipped"

Kairox follows semver. While we are pre-1.0, expect minor releases (`0.X.0`) to include behavior changes. Patch releases (`0.X.Y`) are bug-fix-only. Anything in the "What ships today" section above is in the latest minor release on `main`.

See [Releases & Security](./releases-and-security) for the release model, artifact verification, and the security disclosure flow.

## What this page does not cover

This page is a curated highlight reel. It does not cover individual PR-level history (see [`ROADMAP.md`](https://github.com/Z-Only/kairox/blob/main/ROADMAP.md) and [Releases](https://github.com/Z-Only/kairox/releases)), the contribution workflow ([Contributing](./contributing)), or how to get a security issue fixed ([Releases & Security](./releases-and-security)).

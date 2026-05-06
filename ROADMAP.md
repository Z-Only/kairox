# Roadmap

## Near term

- ✅ Implement memory layer with durable/session/user/workspace scopes
- ✅ Add GUI trace visualization (TraceEntry, TraceTimeline, useTraceStore)
- ✅ Integrate memory into TUI and runtime with marker protocol
- ✅ Add real model adapters (OpenAI, Anthropic, Ollama) via agent-config
- ✅ Interactive permission mode with per-request approval
- ✅ Harden CI/CD: parallel jobs, type-sync checks, shared Rust cache
- ✅ Integrate tauri-specta for auto-generated TypeScript command bindings
- ✅ Complete the GUI shell with full session management UX (persistent storage, switching, rename, delete, startup recovery)
- ✅ Add task graph visualization and inspection in both TUI and GUI (TaskSteps component, density mode, event-driven refresh)
- ✅ Expand test coverage: integration tests for core, store, runtime, and tools crates
- ✅ Auto-generate EventPayload TypeScript types via specta (beyond command bindings)
- ✅ GUI core interaction polish — cancel session, error notifications, memory browser, code highlighting, real status bar
- ✅ Improve packaging outputs and release metadata (updater support)
- ✅ Expand GUI test coverage to 127 tests across stores, composables, and components
- ✅ Add E2E test infrastructure with Playwright for GUI frontend testing
- ✅ Add TUI app logic integration tests (7 tests via FakeModelClient)
- ✅ Add full-stack runtime integration tests (13 tests covering workspace, session, messaging, tools, permissions, memory, persistence)
- ✅ Wire MCP tool execution (client protocol, process lifecycle, config-driven servers)
- ✅ Add fs.write and fs.list built-in tools for filesystem operations
- ✅ Add E2E test job to CI workflow for automated frontend testing
- ✅ Implement Phase 2 DAG execution with AgentStrategy for multi-agent orchestration (#51)
- ✅ Add JSON Schema parameters to tools and CancellationToken for streaming cancellation (#48)
- ✅ Refactor facade_runtime into focused modules (Phase 1) for maintainability (#50)
- ✅ Add agent attribution, N-level task tree visualization, and DAG event handling in GUI (#54)
- ✅ Refresh brand assets and visual identity (#52)
- ✅ Standardize worktree convention documentation (#53)
- ✅ Suppress CI warnings for v-html ESLint rule and Node.js 20 deprecation (#49)

## Mid term

- Support more model providers and profile policies
- ✅ Add multi-agent orchestration UX in TUI and GUI
- Expand MCP ecosystem coverage (more transports, richer discovery, server marketplace UX)
- Improve extension and manifest discovery flows
- Add better observability, tracing, and diagnostics tools
- Continue runtime modularization (Phase 2+ extraction beyond `facade_runtime` split)

## Long term

- More complete application shell for local-first agent operations
- Stronger plugin ecosystem and extension story (built on top of MCP + tool registry)
- Cross-platform desktop distribution polish and auto-update support
- Telemetry-free privacy story with `minimal_trace` defaults in production

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
- ✅ Add full-stack runtime integration tests (12 tests covering workspace, session, messaging, tools, permissions, memory, persistence)

## Mid term

- Support more model providers and profile policies
- Add multi-agent orchestration UX in TUI and GUI
- Improve extension and manifest discovery flows
- Add better observability, tracing, and diagnostics tools
- Wire MCP tool execution (client protocol, process lifecycle, config-driven servers)

## Long term

- More complete application shell for local-first agent operations
- Stronger plugin ecosystem and extension story
- Cross-platform desktop distribution polish and auto-update support

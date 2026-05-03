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
- Add richer session state visualization and task graph inspection (TUI session switching now persist across restarts)
- Improve packaging outputs and release metadata (updater support)

## Mid term

- Support more model providers and profile policies
- Add multi-agent orchestration UX in TUI and GUI
- Improve extension and manifest discovery flows
- Add better observability, tracing, and diagnostics tools
- Expand test coverage: integration tests for core crates, GUI component tests
- Auto-generate EventPayload TypeScript types (beyond command bindings)

## Long term

- More complete application shell for local-first agent operations
- Stronger plugin ecosystem and extension story
- Cross-platform desktop distribution polish and auto-update support

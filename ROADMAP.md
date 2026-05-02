# Roadmap

## Near term

- ✅ Implement memory layer with durable/session/user/workspace scopes
- ✅ Add GUI trace visualization (TraceEntry, TraceTimeline, useTraceStore)
- ✅ Integrate memory into TUI and runtime with marker protocol
- ✅ Add real model adapters (OpenAI, Anthropic, Ollama) via agent-config
- ✅ Interactive permission mode with per-request approval
- Complete the GUI shell with full session management UX
- Add richer session state visualization and task graph inspection
- Improve packaging outputs and release metadata (updater support)
- Harden CI/CD: parallel jobs, type-sync checks, shared Rust cache

## Mid term

- Support more model providers and profile policies
- Add multi-agent orchestration UX in TUI and GUI
- Improve extension and manifest discovery flows
- Add better observability, tracing, and diagnostics tools
- Expand test coverage for agent-store, integration tests for core crates

## Long term

- More complete application shell for local-first agent operations
- Stronger plugin ecosystem and extension story
- Cross-platform desktop distribution polish and auto-update support

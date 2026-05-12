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
- ✅ Add aggregation `ci-success` job for branch protection compatibility
- ✅ Add tests for DAG executor, AgentStrategy, and GUI components (#58)
- ✅ Implement Tauri 2 auto-update with GitHub Release endpoint (#57)
- ✅ MCP marketplace Phase 1 — built-in catalog, installer, GUI (#59)
- ✅ MCP marketplace Phase 2 — remote catalog sources with multi-source aggregation (#60)
- ✅ Migrate frontend toolchain from ESLint + Prettier to Oxc (oxlint + oxfmt) (#100)
- ✅ GUI frontend engineering foundation: vue-router, vue-i18n, Pinia setup stores (#101)
- ✅ Polish GUI display and fix marketplace issues (#102)
- ✅ Use GitHub native auto-merge for Dependabot PRs (#82)
- ✅ Add full-stack desktop testing with tauri-pilot scenarios and CI artifacts (#104)
- ✅ Add live GitHub Models integration smoke test gated behind `live-model-tests` (#104)
- ✅ Add per-model context windows and budget-driven prompt assembly (#105)
- ✅ Add manual and automatic context compaction with busy-state protection (#106)
- ✅ Add GUI context meter and context budget visibility (#107)
- ✅ Add mid-session model switching support (#108)
- ✅ Harden GUI accessibility selectors and tauri-pilot audit coverage
- ✅ Add native skills system for reusable prompt/tool/workflow capabilities
- ✅ Add per-project `.kairox/` config discovery
- ✅ Add project workspace flows in GUI
- ✅ Add MCP and skills settings UI in GUI
- ✅ Optimize build and package pipeline
- ✅ GUI interaction polish and UI primitives improvements

## Mid term

- Support more model providers and profile policies
- ✅ Add multi-agent orchestration UX in TUI and GUI
- ✅ MCP server marketplace UX (Phase 1 + 2 shipped in v0.16.0)
- ✅ Improve GUI UX with richer interaction patterns, accessibility, and explainable agent state
- Continue expanding MCP ecosystem coverage (additional transports, richer discovery)
- ✅ Design a first-class **Skills** system for reusable prompt/tool/workflow capabilities
- Design a signed **Plugin** manifest and installation flow that composes with MCP and the tool registry
- Improve extension and manifest discovery flows, including local development and marketplace publishing paths
- Expand subagent execution primitives beyond planner / worker / reviewer into configurable specialist roles
- Add better observability, tracing, diagnostics, and replay tools for long-running agent work
- Continue runtime modularization (Phase 2+ extraction beyond `facade_runtime` split)

## Long term

- Mature local-first AI agent workbench for planning, execution, review, and recovery
- Strong **Skills** ecosystem with composable workflows, reusable instructions, and capability discovery
- Strong **Plugin** ecosystem and extension story built on top of MCP, the tool registry, signed manifests, and marketplace governance
- Rich subagent and multi-agent collaboration: delegation, arbitration, specialist teams, shared memory, and auditable handoffs
- Cross-platform desktop distribution polish and auto-update support
- Telemetry-free privacy story with `minimal_trace` defaults in production

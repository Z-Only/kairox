# Kairox Roadmap

> Living document. Updated as milestones ship or priorities shift.
> Current version: **0.37.0** (2026-06-05).

## Design principles

1. **Local-first** — data stays on disk; cloud providers are pluggable backends, not lock-in.
2. **Loop × Tools × Memory × Safety × Autonomy** — the five axes of agent capability; advance all, not just one.
3. **Trait-based boundaries** — every capability is behind an interface; swap, mock, or extend without rewiring.
4. **Dual interface** — TUI for power users, GUI for everyone; same runtime underneath.
5. **Ship incrementally** — each milestone is independently useful; no big-bang releases.

---

## Capability maturity matrix

Where Kairox stands relative to industry agents (Claude Code, Codex CLI, OpenCode, Aider, Cline, Goose, Continue).

| Capability                                | Kairox status                                         | Industry best-in-class                                               |
| ----------------------------------------- | ----------------------------------------------------- | -------------------------------------------------------------------- |
| Agent loop + tool dispatch                | ✅ Event-sourced session actors                       | Claude Code, Codex CLI                                               |
| Multi-agent orchestration                 | ✅ Planner/Worker/Reviewer + DAG executor             | Ahead of most; Devin-style long-running is next frontier             |
| MCP integration                           | ✅ stdio/SSE/Streamable HTTP + marketplace            | Claude Code parity                                                   |
| Tool safety (approval + sandbox)          | ✅ Orthogonal ApprovalPolicy × SandboxPolicy          | Ahead of most; Codex CLI has similar sandbox                         |
| Prompt caching                            | ✅ Cache breakpoints + hit-rate tracking              | Claude quickstarts best-practices shows optimal breakpoint placement |
| Memory + context assembly                 | ✅ Multi-scope memory + tiktoken budgets + compaction | Competitive; RAG retrieval is gap                                    |
| Skills + plugins                          | ✅ Native skills + plugin manifests + marketplace     | Ahead of most OSS agents                                             |
| Eval harness                              | ✅ JSONL scenarios + tag filters + expectations       | Competitive with SWE-bench style harnesses                           |
| LSP / code intelligence                   | ✅ Native LSP + DAP                                   | OpenCode, Continue have similar                                      |
| Server-side tools (code exec, web search) | ✅ code_execution + web_search passthrough            | Claude Code, Codex use provider-hosted tools                         |
| Multimodal context management             | ✅ Image pruning strategies                           | Claude quickstarts has reference impl                                |
| Browser / computer use                    | ✅ Browser tool + Computer use primitives             | Claude quickstarts, browser-use, Cline                               |
| Trajectory recording                      | ✅ Runtime auto-capture + GUI viewer                  | SWE-Agent, Moatless                                                  |
| Agent self-reflection / advisor           | ✅ Inline advisor review before tool execution        | Anthropic BetaAdvisorTool (beta)                                     |
| Long-running autonomous mode              | ✅ Autonomous controller + session chaining           | Claude quickstarts autonomous-coding, Codex background tasks         |
| Embedded SDK mode                         | ✅ `agent-sdk` crate with builder + streaming         | Claude Agent SDK, Goose extensible-agent                             |
| Streaming UX                              | ⚠️ Basic event forwarding                             | Claude Code, Codex CLI have rich streaming                           |
| Git-aware workflows                       | ⚠️ Basic shell.exec                                   | Aider, Claude Code have deep git integration                         |

---

## Phase 1 — Foundation hardening (v0.35 – v0.36) ✅

Low-risk, high-leverage improvements to the existing architecture.

### 1.1 Prompt cache optimization ✅

**Crate**: `agent-models` (Anthropic adapter)

Apply the cache breakpoint strategy from Anthropic's computer-use best-practices:

- System block gets a static cache breakpoint.
- Last N (up to 3) `tool_result` or `compaction` blocks get ephemeral breakpoints.
- Clear stale breakpoints each turn before setting new ones.
- Track and surface cache hit rate in trace output (already have `structured trace export`).

**Why**: Direct cost/latency reduction. The pattern is proven in production (Anthropic's own reference impl). Minimal code change in the model adapter layer.

### 1.2 Server-side tool types ✅

**Crates**: `agent-models`, `agent-tools`, `agent-config`

Support Anthropic's server-hosted tool format:

- `code_execution` (`type: "code_execution_20250522"`) — sandboxed code execution on Anthropic's infra.
- `web_search` (`type: "web_search_20250305"`) — provider-side web search with domain filtering.

These are _not_ local tools — they're API-level tool types that Anthropic executes server-side. The model adapter needs to pass them through correctly, and the config layer needs to let users enable/disable them per profile.

**Why**: Unlocks code execution and web search without running local infrastructure. Claude quickstarts L3 shows this is the lowest-friction path to these capabilities.

### 1.3 Multimodal context management ✅

**Crate**: `agent-memory` (ContextAssembler)

Add image pruning strategies for conversations containing screenshots or uploaded images:

- `StripOldestImages` — keep only the N most recent images.
- `StripImagesAtIntervals` — keep images at regular intervals (1st, every Kth, last).
- Integrate with context budget enforcement.

**Why**: Without this, a single conversation with several screenshots can exhaust the context window. Prerequisite for browser/computer use (Phase 2) and useful immediately for multimodal chat.

### 1.4 Retry and error resilience ✅

**Crate**: `agent-models`

Adopt the retry classification from computer-use best-practices:

- Unrecoverable: `BadRequest`, `Authentication`, `Permission`, `Unprocessable`.
- Recoverable: `RateLimit`, `ConnectionError`, 5xx, "overloaded" in body.
- Exponential backoff with jitter.
- Surface retry attempts in trace.

**Why**: Production agents hit transient errors constantly. Systematic retry is table stakes.

---

## Phase 2 — Environment interaction (v0.37 – v0.39) ✅

The leap from "reads and writes files" to "sees and interacts with running software."

### 2.1 Browser tool (Playwright-backed) ✅

**Crates**: `agent-tools` (built-in or MCP server)

Reference: `claude-quickstarts/browser-use-demo`

Implement a browser automation tool with:

- Playwright backend for cross-browser support.
- Screenshot capture → model sees the page.
- DOM element references (ref-based targeting, more reliable than coordinates).
- Actions: navigate, click, type, scroll, hover, form fill, wait, get page text.
- Screenshot → action → screenshot verification loop.

Can ship as either:

- A built-in tool in `agent-tools` (tighter integration with policy engine).
- An MCP server (more portable, community can reuse).

**Why**: Enables end-to-end UI verification. The autonomous-coding demo shows this is the difference between "code compiles" and "feature actually works." Kairox already has `tauri-pilot` infrastructure for desktop testing; browser tool is the web equivalent.

### 2.2 Batch tool execution ✅

**Crates**: `agent-runtime`, `agent-tools`

Complete. The batch tool execution system now includes:

- **`browser.batch` tool**: Sequences multiple browser actions in a single tool call without intermediate screenshots, reducing API round-trips for deterministic multi-step operations.
- **Structured image attachments**: `ToolOutput.images: Vec<ImageAttachment>` carries screenshots as structured data (media_type + base64 data + optional label) rather than embedding them in truncated text output.
- **Event propagation**: `EventPayload::ToolInvocationCompleted` includes an `images` field that flows through the entire pipeline — runtime → event store → Tauri IPC → GUI rendering.
- **GUI rendering**: `ChatToolCallItem.vue` renders attached images from the structured `images` prop, with legacy regex fallback for older events.
- **Batch reminder**: Model receives a nudge when issuing lone single-action calls that could be batched.

**Why**: 2-5x latency reduction on repetitive tasks. The best-practices reference shows significant cost savings. Structured image attachments also solve the screenshot truncation problem that affected `computer.use` output.

### 2.3 Trajectory recording ✅

**Crates**: `agent-store`, `agent-core`, `agent-runtime`, `agent-gui`

Complete. The trajectory system now includes:

- **Data layer**: `TrajectoryStore` trait + `SqliteTrajectoryStore` with migration `0009_trajectories.sql`.
- **Core types**: `TrajectoryId`, `TrajectoryStep`, `TrajectoryMeta`, `TrajectoryOutcome` in `agent-core`.
- **Events**: `TrajectoryStarted`, `TrajectoryStepRecorded`, `TrajectoryCompleted` in `EventPayload`.
- **Runtime auto-capture**: The agent loop automatically starts a trajectory per turn, records each tool invocation as a step, and completes the trajectory with success/failure/cancelled outcome.
- **Facade API**: `list_trajectories`, `get_trajectory_steps`, `export_trajectory` on `SessionFacade`.
- **GUI viewer**: Trajectory tab in the trace panel with expandable step-by-step inspection and JSON export.

**Why**: Essential for debugging agent behavior and feeding into eval. Closes the gap with SWE-Agent and Moatless trajectory-based debugging.

### 2.4 Computer use (desktop interaction) ✅

**Crate**: `agent-tools`

Complete. The computer use system now includes:

- **Tool**: `computer.use` (`ComputerUseTool`) registered as a built-in tool with `computer.interact` capability.
- **Actions**: `screenshot` (full/region), `mouse_move`, `mouse_click`, `mouse_drag`, `keyboard_type`, `key_press`, `scroll`, `wait`, `get_screen_size`, `get_cursor_position`.
- **Screenshot sizing**: `MODEL_SCREENSHOT_MAX_WIDTH` / `MODEL_SCREENSHOT_MAX_HEIGHT` constants (1280×800) for resizing screenshots to model coordinate space.
- **Platform backend**: `DesktopBackend` with simulated responses; real implementations will use macOS CoreGraphics/ApplicationServices and Linux X11/Wayland.
- **Policy integration**: Reports `ToolEffect::Execute` risk for approval policy enforcement.
- **Tests**: Full unit coverage for all actions, error handling, and defaults.

**Why**: Enables the agent to operate any desktop application, not just code editors. High-value for testing, form filling, and workflow automation. Shipped after browser tool since browser is safer and more commonly needed.

---

## Phase 3 — Autonomy and long-running agents (v0.40 – v0.42) ✅

From session-scoped to task-scoped execution that survives context limits.

### 3.1 Long-running autonomous mode ✅

**Crates**: `agent-runtime`, `agent-core`, `agent-store`, `agent-gui`

Complete. The autonomous mode system now includes:

- **Core types**: `AutonomousTaskState`, `AutonomousTaskGoal`, `AutonomousConfig`, `AutonomousTaskId`, `VerificationResult`, `SessionEndReason`, `Checkpoint` in `agent-core`.
- **Data layer**: `AutonomousTaskStore` trait + `SqliteAutonomousTaskStore` with migration `0010_autonomous_tasks.sql`. Supports task rows, checkpoint rows, and session chain tracking.
- **Controller**: `AutonomousController` in `agent-runtime` orchestrates the full lifecycle — task creation, session-end handling, auto-continue with fresh sessions, checkpoint persistence, and verification.
- **Orientation**: `OrientationPromptBuilder` generates fresh-context orientation prompts for continuation sessions (read progress, check git, verify previous work).
- **Checkpointing**: `CheckpointWriter` persists structured progress between sessions with git checkpoint support.
- **Events**: `AutonomousTaskCreated`, `AutonomousTaskSessionStarted`, `AutonomousTaskSessionEnded`, `AutonomousTaskCompleted`, `AutonomousTaskCheckpointed`, `AutonomousTaskVerificationRan` in `EventPayload`.
- **Facade**: `AutonomousFacade` trait with `start_autonomous_task`, `pause_autonomous_task`, `resume_autonomous_task`, `cancel_autonomous_task`, `list_autonomous_tasks`, `get_autonomous_task` — implemented by `LocalRuntime`.
- **GUI integration**: Tauri commands, Pinia store, `AutonomousSettingsPane.vue` for configuration, generated TypeScript bindings.
- **Tests**: Full coverage for controller lifecycle, orientation prompt building, checkpoint writing, facade operations, and store persistence.

**Why**: This is the defining capability gap between "copilot" and "autonomous agent." The claude-quickstarts autonomous-coding demo proves the pattern works with a simple harness; Kairox's richer infrastructure (event store, multi-agent, DAG) does it better.

### 3.2 Kairox SDK (embedded runtime) ✅

**Crate**: `agent-sdk`

Complete. The SDK exposes the Kairox runtime as an embeddable, programmatic API:

- **Builder API**: `KairoxSdk::builder()` with fluent configuration — workspace path, data/home dir, database filename, default profile, approval/sandbox policies, MCP/LSP/marketplace toggles.
- **Session management**: `create_session()`, `create_session_with_profile()`, `list_sessions()` — programmatic session lifecycle without a UI.
- **Message streaming**: `session.send_message()` returns a `MessageStream` implementing `futures::Stream<Item = StreamEvent>`. `StreamEvent` variants: `Text`, `ToolCall`, `ToolResult`, `TurnCompleted`, `Error`, `Other`.
- **Collected responses**: `stream.collect_all()` gathers the full assistant text and event log in one call.
- **Hook system**: `SdkHook` trait with `before_tool` / `after_tool` callbacks. `before_tool` returns `HookAction::Continue` or `HookAction::Reject(reason)` to gate tool execution.
- **Security settings API**: `SdkApprovalPolicy` (Never/OnRequest/Always) and `SdkSandboxPolicy` (ReadOnly/WorkspaceWrite/FullAccess) map to the runtime's orthogonal policy engine.
- **Facade access**: `sdk.facade()` exposes the full `AppFacade` for advanced use cases.
- **Tests**: 15 unit tests + 3 doc tests covering builder validation, session creation, policy conversion, stream event mapping, hook registration, and error variants.

Use cases:

- External harnesses driving Kairox for autonomous coding.
- CI/CD integration (run agent tasks as pipeline steps).
- Custom UIs beyond TUI/GUI.

**Why**: Claude Agent SDK (`claude_code_sdk`) enables the autonomous-coding demo by wrapping Claude Code as a programmable runtime. Kairox now offers the same. Codex CLI's `--quiet` mode and API-driven execution serve a similar purpose.

### 3.3 Agent self-reflection / advisor ✅

**Crates**: `agent-core`, `agent-config`, `agent-runtime`

Complete. The advisor (self-reflection) system now includes:

- **Core types**: `AdvisorMode` (`Off` / `Lightweight` / `Full`), `AdvisorVerdict` (`Approve` / `ApproveWithWarnings` / `Reject`), `AdvisorConcern`, `AdvisorReview` in `agent-core`.
- **Configuration**: `AdvisorConfig` in `agent-config` with `[advisor]` TOML section — mode, profile alias (use a cheaper model like Haiku), and max concerns cap.
- **Risk heuristics**: `should_review()` + `is_high_risk_tool_call()` in `agent-runtime` — Lightweight mode only triggers on destructive shell commands, sensitive file writes, and computer-use actions.
- **Review flow**: `review_tool_calls()` sends the planned tool calls + agent reasoning to the advisor model, parses the structured JSON response, and returns an `AdvisorReview`.
- **Agent loop integration**: Advisor review runs inline in the agent loop between model response and tool execution. On `Reject` verdict, tool execution is blocked and the rejection is surfaced as an assistant message. On `Approve` / `ApproveWithWarnings`, execution proceeds normally.
- **Events**: `AdvisorReviewStarted` and `AdvisorReviewCompleted` in `EventPayload` for trace visibility.
- **Graceful degradation**: Advisor failures (model errors, unparseable responses) default to `Approve` — the advisor never blocks the main agent loop due to its own issues.
- **Tests**: 21 unit tests covering mode gating, risk heuristics, response parsing, FakeModelClient integration, and concern truncation.

**Why**: As agents take more autonomous actions, a built-in "second opinion" reduces costly mistakes. The advisor can use a cheaper/faster model (e.g., Haiku reviewing Sonnet's plan) to keep latency and cost low while catching destructive operations.

---

## Phase 4 — Knowledge and retrieval (v0.43+)

### 4.1 Workspace RAG (vector retrieval) ✅

**Crates**: `agent-memory`

Add embedding-based retrieval for workspace documents:

- ✅ Index project files, documentation, past conversations.
- ✅ Retrieval at context assembly time — inject relevant chunks.
- ✅ Pluggable embedding backend (local deterministic backend now; `fastembed` / remote API can implement the same trait).
- ✅ Incremental index updates on file change.

**Implemented**: `agent-memory` now provides `WorkspaceRagIndex`, a SQLite-backed vector chunk store, pluggable `EmbeddingBackend`, workspace-scoped retrieval, and runtime context injection with a distinct `workspace_retrieval` context source.

**Why**: Current memory is key-value scoped. RAG enables "find relevant context I didn't explicitly save." The customer-support-agent demo uses Bedrock KB for this; Kairox should offer a local-first alternative.

### 4.2 Git-aware context

**Crates**: `agent-tools` or `agent-memory`

Deep git integration beyond shell commands:

- Automatic diff context injection (what changed recently, what's staged).
- Branch-aware memory scoping.
- Commit message generation from conversation context.
- PR description drafting.
- Blame-informed context (who wrote this, when, why).

**Why**: Aider's strongest differentiator is deep git integration. Claude Code does this via hooks and slash commands. Kairox should make git state a first-class input to the agent, not just a tool the agent calls.

### 4.3 External knowledge base connectors

**Crates**: `agent-memory`, `agent-config`

Pluggable KB connectors:

- Local: SQLite FTS, tantivy.
- Cloud: Bedrock Knowledge Bases, Pinecone, Weaviate.
- Config-driven: specify KB sources per profile.

**Why**: Extends RAG beyond the local workspace. Enterprise use cases need organizational knowledge.

---

## Phase 5 — Production hardening (ongoing)

### 5.1 Streaming UX improvements

- Token-by-token streaming in both TUI and GUI.
- Thinking/reasoning display (extended thinking blocks).
- Progressive tool result rendering.
- Interrupt/cancel mid-stream.

### 5.2 Cost and usage tracking

- Per-session and per-task token usage with cost estimates.
- Budget limits (stop after $X or N tokens).
- Usage dashboard in GUI.

### 5.3 Telemetry and observability

- OpenTelemetry spans for agent loop, tool calls, model requests.
- Structured logging with correlation IDs.
- Performance profiling for long sessions.

### 5.4 Multi-provider parity

- Ensure all Phase 1-3 features work across OpenAI, Anthropic, Ollama, and other providers.
- Provider-specific optimizations (prompt caching for Anthropic, function calling format for OpenAI).
- Graceful degradation when provider doesn't support a feature (e.g., no server tools on Ollama).

---

## Industry reference map

Key insights extracted from industry agents that inform this roadmap:

| Agent                    | Key lesson for Kairox                                                                                                                           |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| **Claude Code**          | Hooks, slash commands, and CLAUDE.md as config surface. Kairox skills/plugins cover similar ground.                                             |
| **Claude quickstarts**   | Five-layer capability ladder (loop → RAG → multimodal → environment → autonomy). Kairox is at layer 1-2, this roadmap targets layer 3-5.        |
| **Codex CLI**            | Network-disabled sandbox by default, background task execution, `--quiet` mode for CI. Kairox has sandbox; needs background tasks and SDK mode. |
| **OpenCode**             | Clean TUI with LSP integration, built-in git diff display. Kairox TUI is competitive; LSP landed in v0.34.                                      |
| **Aider**                | Deep git integration (auto-commit, repo map, blame context). Kairox should treat git as first-class context source, not just a tool.            |
| **Cline**                | VS Code extension with browser preview, terminal integration. Kairox GUI serves similar role; browser tool would close the gap.                 |
| **Goose**                | Extensible agent with pluggable "toolkits." Similar to Kairox plugins/skills but less structured.                                               |
| **Continue**             | IDE-embedded with autocomplete + chat + edit modes. Different UX paradigm; Kairox focuses on autonomous agent sessions.                         |
| **SWE-Agent / Moatless** | Trajectory-based debugging and eval. Kairox now has full trajectory recording: runtime auto-capture + GUI inspection + JSON export.             |

---

## Non-goals

Things explicitly out of scope for the foreseeable future:

- **Cloud-hosted agent service** — Kairox is local-first. No SaaS offering planned.
- **IDE extension** — TUI and GUI are the interfaces. IDE integration happens via MCP servers and project config, not embedded plugins.
- **Model training / fine-tuning** — Kairox is an agent runtime, not a model provider.
- **Competing with model providers on server tools** — Use Anthropic's code execution and web search rather than reimplementing them.

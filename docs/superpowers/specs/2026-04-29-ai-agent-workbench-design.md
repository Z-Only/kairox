# Cross-Platform AI Agent Workbench Design

## Summary

Build a local-first, cross-platform developer AI Agent workbench. The product ships two user interfaces: a Ratatui TUI and a Tauri/Vue GUI. Both clients share the same Rust core library for agent sessions, model routing, tools, memory, permissions, trace, and audit.

The first release focuses on a developer workbench similar in spirit to code-oriented agent tools: repository understanding, command execution, file editing, patch review, project memory, MCP tools, and lightweight multi-agent orchestration. Login is optional and only unlocks cloud conveniences such as settings sync and subscription lookup. Users who never log in still get full local functionality with bring-your-own API keys and local models.

## Product Scope

### Phase 1

Phase 1 is a local-first MVP:

- Shared Rust Core Library used by both TUI and GUI.
- Ratatui TUI for terminal-first workflows.
- Tauri/Vue GUI for richer trace, settings, and workspace views.
- OpenAI-compatible model providers and Ollama as first-class model adapters.
- Optional login boundary, not required for core software use.
- Four permission modes for local tool execution.
- Local project memory without mandatory embeddings.
- MCP client integration for local tool servers.
- Manifest-only Skill and Plugin discovery.
- Lightweight multi-agent orchestration with planner, worker, and reviewer roles.
- Event trace and audit store from the first version.

### Deferred Scope

These are intentionally not in Phase 1:

- Full plugin marketplace, signing, updating, and third-party code execution.
- Complete visual DAG workflow editor.
- Team collaboration and remote agent fleets.
- Required cloud account, required cloud storage, or required subscription login.
- Mandatory semantic indexing or embedding service.
- Enterprise policy server.

## Architecture

The core architecture is an evented runtime behind a stable application facade.

```text
Ratatui TUI          Tauri/Vue GUI
     |                    |
     +---- App Facade ----+
              |
       Evented Runtime
              |
  +-----------+-----------+-----------+
  |           |           |           |
Sessions   Models      Tools       Memory
  |           |           |           |
Trace      Store       MCP       Context
```

The TUI and GUI do not own agent business logic. They send commands, query projections, and subscribe to event streams through the App Facade. The runtime appends domain events and updates session projections. This keeps streaming model output, tool approval, cancellation, trace replay, and multi-agent orchestration consistent across both interfaces.

## Rust Workspace Layout

Use a Rust workspace with focused crates:

- `agent-core`: shared domain types, event schemas, session state, App Facade traits, error types.
- `agent-runtime`: agent loop, multi-agent orchestration, cancellation, pause/resume, runtime scheduler.
- `agent-models`: model provider traits, OpenAI-compatible adapter, Ollama adapter, capability matrix, model profiles.
- `agent-tools`: local filesystem tools, search, patch application, git helpers, shell execution, MCP client integration.
- `agent-memory`: user memory, workspace memory, session memory, context assembly interfaces, semantic index trait.
- `agent-store`: SQLite and local file persistence for config, sessions, events, traces, and memory.
- `agent-tui`: Ratatui application, terminal input handling, event subscription, local projections.
- `agent-gui`: Tauri/Vue application, Tauri command bridge, GUI projections and settings screens.

The core crates should be UI-agnostic. UI code may depend on App Facade types, but core crates must not depend on Ratatui, Vue, or Tauri-specific UI concepts.

## Event Model

All important runtime activity becomes an append-only event. Initial event families:

- `WorkspaceOpened`
- `UserMessageAdded`
- `AgentTaskCreated`
- `AgentTaskStarted`
- `ContextAssembled`
- `ModelRequestStarted`
- `ModelTokenDelta`
- `ModelToolCallRequested`
- `PermissionRequested`
- `PermissionGranted`
- `PermissionDenied`
- `ToolInvocationStarted`
- `ToolInvocationCompleted`
- `ToolInvocationFailed`
- `FilePatchProposed`
- `FilePatchApplied`
- `MemoryProposed`
- `MemoryAccepted`
- `MemoryRejected`
- `ReviewerFindingAdded`
- `AssistantMessageCompleted`
- `AgentTaskCompleted`
- `AgentTaskFailed`
- `SessionCancelled`

Events must include schema version, workspace id, session id, timestamp, source agent id, and privacy classification. The runtime projects these events into UI state such as chat messages, task queues, trace timelines, permission prompts, and review findings.

## Application Facade

The App Facade is the only supported boundary for UI clients. It exposes:

- Commands: open workspace, start session, send message, approve tool, deny tool, cancel task, apply patch, update config.
- Queries: list workspaces, get session projection, get trace, list model profiles, list tools, list memories.
- Subscriptions: session event stream, task status stream, permission prompt stream, model token stream.

The facade hides storage format, internal event replay, model adapter details, and tool execution internals. This allows the TUI and GUI to evolve independently without forking core behavior.

## Model Layer

Phase 1 uses a dual strategy:

- `openai_compatible`: configurable `base_url`, `api_key`, model id, headers, and capability overrides.
- `ollama`: local model discovery, health checks, model selection, and local generation options.

The abstraction should not collapse all vendors into a lowest-common-denominator chat API. Use:

- `ModelProvider`: lists models, validates credentials, creates clients.
- `ModelClient`: runs complete and streaming requests.
- `ModelRequest`: normalized request containing messages, tools, response format, temperature, limits, and metadata.
- `ModelEvent`: streaming output, reasoning summaries where available, tool calls, errors, usage, and completion.
- `ModelCapabilities`: streaming, tool calling, JSON schema, vision, reasoning controls, context window, output limit, local model flag.
- `ModelProfile`: user-defined aliases such as `fast`, `deep-reasoning`, `local-code`, and `reviewer`.

Anthropic and Google native adapters are reserved for later phases. The core types must leave room for provider-specific metadata without leaking provider-specific types into UI code.

## Tools, MCP, And Permissions

All tools are registered through a unified `ToolRegistry`. Phase 1 tool families:

- Filesystem read and write within workspace policy.
- Ripgrep-style search.
- Git status and diff.
- Patch proposal and application.
- Shell command execution.
- MCP tool invocation.

Tool invocations use a common envelope:

```text
tool_id
arguments
workspace_id
risk_level
required_capability
preview
approval_state
timeout
output_limit
```

MCP servers are local tool providers. MCP tools map to `ToolDefinition`; MCP resources map to context sources; MCP prompts map to Skill or prompt sources. The first release should support local server configuration, tool discovery, and invocation through the same permission pipeline as built-in tools.

### Permission Modes

Use four permission modes:

- `Read-only`: read permitted files, inspect git state, search workspace. No writes or shell execution.
- `Suggest`: generate patches and command suggestions. User approval required before execution.
- `Agent`: execute policy-approved commands and writes. Escalating actions require approval.
- `Autonomous`: continue within workspace policy without prompting for every action. Destructive actions, sensitive paths, external network use, and policy escape still require approval.

The `PermissionEngine` owns decisions. UI clients render prompts and send decisions, but they do not interpret safety rules. Every permission decision is recorded in the event stream.

## Memory And Context

Phase 1 implements local project memory:

- `user_memory`: durable user preferences and global operating rules.
- `workspace_memory`: project summaries, important files, common commands, tech stack notes, repository conventions.
- `session_memory`: short-lived facts and decisions from the current session.

Memory writes happen through `MemoryProposed` events. The default behavior is user confirmation for durable memory and automatic acceptance only for low-risk session memory.

`ContextAssembler` builds model input from:

- Current user request.
- Session history.
- Selected workspace files.
- Recent tool results.
- Active task state.
- Applicable user and workspace memory.
- Model capability and context window.
- Current permission mode.

Phase 1 uses explicit references and rule-based context selection. It defines a `SemanticIndex` trait for later embedding-based retrieval and symbol-aware indexing.

## Multi-Agent Orchestration

Phase 1 supports lightweight multi-agent orchestration:

- `PlannerAgent`: decomposes work, chooses model profile, selects tools, creates tasks.
- `WorkerAgent`: performs focused tool-using work such as file reads, edits, shell commands, and MCP calls.
- `ReviewerAgent`: checks diffs, risks, tests, permissions, and alignment with the user request.

The runtime maintains a task graph with these states:

- `pending`
- `running`
- `blocked`
- `completed`
- `failed`
- `cancelled`

Workers may run in parallel when task dependencies and permission policy allow it. The reviewer can run at explicit checkpoints or after risky changes. All agent messages, tool calls, model choices, and review findings write into the same trace stream.

Phase 1 does not include a full workflow canvas. UI should show a task queue, task details, and trace timeline.

## Skill And Plugin System

Phase 1 uses manifest-only extensions. Local folders can declare `skill.toml` or `plugin.toml` with:

- id
- name
- description
- version
- trigger conditions
- prompt templates
- required tools
- required permission capabilities
- compatible core version range

The runtime can discover, index, display, and let agents reference these extensions during planning. It does not execute third-party extension code in Phase 1.

Phase 2 may add script plugins using a controlled tool API, explicit permissions, versioning, isolation, and update policy.

## Account, Login, And Sync

Login is an enhancement layer, not a prerequisite.

Without login, users can:

- Configure local and API-key models.
- Use TUI and GUI.
- Run local tools within policy.
- Use MCP servers.
- Store local settings.
- Store local memory.
- Inspect local trace and audit logs.

With login, users may enable:

- Settings sync.
- Subscription plan lookup.
- Cloud model entitlements.
- Cross-device model profiles.
- Optional encrypted cloud backup.

Core exposes an `AccountService` trait. A local no-account implementation is always present. Cloud implementations must not become a dependency of the core runtime path.

## Storage

Use local storage by default:

- SQLite for events, sessions, traces, memory metadata, model profiles, tool registry cache, and settings.
- Workspace-local files for project-specific configuration where useful.
- OS keychain or platform credential store for API keys and login tokens.

Privacy modes:

- `minimal_trace`: store event metadata, redacted content, command summaries, exit codes.
- `full_trace`: store full model inputs, outputs, tool arguments, and tool outputs.

The default should be privacy-preserving enough for source-code work.

## UI Responsibilities

### TUI

The TUI should optimize for keyboard-heavy developer workflows:

- Chat/session pane.
- Task queue pane.
- Trace/log pane.
- Permission prompt modal.
- Diff preview.
- Model/profile selector.
- Workspace status bar.

### GUI

The GUI should optimize for inspection, settings, and richer visualization:

- Session list.
- Chat and task detail view.
- Trace timeline.
- Permission center.
- Model provider settings.
- Memory editor.
- Skill/Plugin manifest browser.
- MCP server configuration.

Both UIs consume the same projections and commands from the App Facade.

## Testing Strategy

Rust core tests:

- Event schema serialization and migration.
- Session state projection.
- Permission mode decisions.
- Tool risk classification.
- Fake model provider streaming.
- OpenAI-compatible request mapping.
- Ollama model discovery mock.
- Context assembly.
- Memory proposal rules.
- Multi-agent task graph scheduling.

TUI tests:

- Projection-to-view rendering tests.
- Keyboard command routing.
- Permission prompt state tests.

GUI tests:

- Tauri command contract tests.
- Vue component tests for trace, settings, and permission flows.
- Projection fixture tests.

End-to-end tests:

- Fake model provider completes a full session.
- Tool approval flow writes expected events.
- Patch proposal and application flow.
- Reviewer detects a risky change.
- Trace replay reconstructs session state.

## Milestones

### M0: Foundation

- Rust workspace scaffold.
- Core event types.
- Store abstraction.
- Fake model provider.
- App Facade skeleton.

### M1: TUI Single-Agent Loop

- Ratatui shell.
- Start session and send message.
- Stream fake model output.
- Render event projection.

### M2: Models, Tools, And Permissions

- OpenAI-compatible adapter.
- Ollama adapter.
- Filesystem, search, git, shell, and patch tools.
- Four permission modes.
- Permission prompt flow.

### M3: GUI Shell

- Tauri/Vue shell.
- Tauri command bridge.
- Session, trace, settings, and permission views.
- Shared App Facade integration.

### M4: Memory, MCP, And Manifests

- Local project memory.
- Context assembler.
- MCP local server integration.
- Skill/Plugin manifest discovery.

### M5: Lightweight Multi-Agent Runtime

- Planner, worker, reviewer roles.
- Task graph.
- Parallel worker scheduling where safe.
- Review checkpoints.
- Trace and audit completion.

### M6: Optional Account Layer

- AccountService trait.
- Local no-account implementation.
- Cloud account adapter.
- Settings sync.
- Subscription lookup.

## Implementation Defaults

Use these defaults unless implementation reveals a concrete reason to change them:

- Use SQLite with `sqlx` migrations for local structured storage.
- Expose the App Facade as Rust traits and async Rust APIs first. Add a local JSON-RPC boundary only after the TUI and GUI prove the facade shape.
- Keep `agent-gui` in the same repository and cargo workspace, with the Vue frontend nested under the Tauri app directory.
- Use a fake in-process MCP server fixture first, then add filesystem and git MCP fixture servers for integration tests.
- Define `skill.toml` and `plugin.toml` with shared manifest fields: `id`, `name`, `version`, `description`, `triggers`, `prompt_templates`, `required_tools`, `required_permissions`, and `core_version`.

## Acceptance Criteria

The design is successful when:

- TUI and GUI can run the same agent session through the same Rust core.
- Model adapters can be added without changing UI code.
- Tool calls always pass through the permission engine.
- Users can use the product without login.
- Project memory improves context without requiring embeddings.
- MCP tools share the same invocation and audit path as built-in tools.
- Multi-agent orchestration is useful without requiring a visual workflow canvas.
- Trace replay can reconstruct meaningful session state for debugging and audit.

# Runtime Skill Invocation and Stream Stall Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` or `superpowers:executing-plans` to implement this plan task-by-task.

## Goal

Make Kairox project conversations capable of reliably invoking project-local skills through slash commands, then prevent model stream stalls from leaving a session actor permanently busy.

## Acceptance Signals

- GUI `send_message` treats `/skill-id task` as manual skill activation when the skill exists, sends only `task` to the model, and preserves the slash text as display content.
- Project skill discovery includes both `.kairox/skills` and `.agents/skills` for the active project/worktree.
- A model stream that never opens or never yields an event eventually fails the root task and releases the active turn instead of leaving the planner `Running` forever.
- Existing `/goal` handling and ordinary messages remain compatible.

## Implementation Tasks

### Task 1: GUI Slash Skill Dispatch

Files:

- `apps/agent-gui/src-tauri/src/commands/chat.rs`

Steps:

- [x] Add a pure parser for `/skill-id task`.
- [x] Before attachment enrichment, resolve current session skill roots, check whether the skill exists, activate it, and replace model content with the task body.
- [x] Keep `/goal` behavior first in precedence.
- [x] Leave unknown slash text unchanged.
- [x] Add tests for valid slash activation, malformed slash input, unknown slash input, and existing attachment/goal behavior.

### Task 2: Project `.agents/skills` Discovery

Files:

- `crates/agent-runtime/src/skills.rs`
- `crates/agent-runtime/src/skills_tests.rs`
- `crates/agent-runtime/tests/skills_context.rs`

Steps:

- [x] Add workspace `.agents/skills` to default skill discovery roots.
- [x] Derive a sibling `.agents/skills` discovery root from project `.kairox/skills` settings roots while leaving install/settings writes on `.kairox/skills`.
- [x] Add integration coverage proving a project draft session can activate and inject a `.agents/skills` skill into model context.

### Task 3: Model Stream Stall Handling

Files:

- `crates/agent-runtime/src/agent_loop/stream_handler.rs`
- `crates/agent-runtime/src/agent_loop/stream_handler_tests.rs`

Steps:

- [x] Add deterministic timeout coverage for a model stream that never opens.
- [x] Add deterministic timeout coverage for a model stream that opens but never yields an event.
- [x] Route timeout through the model-failure path so the root task is marked failed and later turns can proceed.
- [x] Use a long production timeout to avoid treating normal provider quiet periods as failures; keep short timeouts only in tests.

## Verification

Focused gates:

```bash
cargo test -p agent-runtime stream_ --lib
cargo test -p agent-runtime project_session_discovers --test skills_context
cargo test -p agent-gui-tauri chat_attachment_tests
```

Pre-merge gates:

```bash
bun run format:check
bun run lint
```

Dev App smoke:

```bash
bun --filter agent-gui tauri dev --features pilot
tauri-pilot ping
tauri-pilot logs --level error
```

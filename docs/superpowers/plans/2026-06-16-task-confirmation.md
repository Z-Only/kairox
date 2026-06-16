# Task Confirmation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add structured task clarification / confirmation requests that pause an ambiguous agent turn, render selectable options in GUI and TUI chat, and resume after the user submits selected options or custom text.

**Architecture:** Add event-sourced `TaskConfirmationRequested` / `TaskConfirmationResolved` payloads and a `TaskConfirmationDecision` facade DTO. Runtime exposes a built-in model tool definition named `task_confirmation.request`, intercepts it in the tool loop, emits the pending event, waits on an oneshot map, then returns the user response as the tool result. GUI and TUI render the pending event from the chat stream and resolve it through new facade / IPC commands.

**Tech Stack:** Rust `agent-core`, `agent-runtime`, `agent-tui`; Vue 3 + Pinia `agent-gui`; Specta type generation; Vitest and Cargo tests.

---

### Task 1: Core Event And Facade Contract

**Files:**

- Modify: `crates/agent-core/src/events.rs`
- Modify: `crates/agent-core/src/facade/session.rs`
- Modify: `crates/agent-core/src/facade.rs`
- Modify: `crates/agent-core/src/lib.rs`
- Test: `crates/agent-core/tests/event_roundtrip.rs`
- Test: `crates/agent-core/tests/event_coverage.rs`

- [ ] **Step 1: Write failing event/facade tests**

Add round-trip coverage for:

```rust
EventPayload::TaskConfirmationRequested {
    request_id: "clarify-1".into(),
    prompt: "Which files should I touch?".into(),
    options: vec![
        TaskConfirmationOption { id: "tests".into(), label: "Tests only".into(), description: Some("Add failing tests first".into()) },
        TaskConfirmationOption { id: "impl".into(), label: "Implementation".into(), description: None },
    ],
    allow_multiple: true,
    allow_custom: true,
}
```

and:

```rust
EventPayload::TaskConfirmationResolved {
    request_id: "clarify-1".into(),
    selected_option_ids: vec!["tests".into()],
    custom_response: Some("Also update TUI".into()),
}
```

- [ ] **Step 2: Run RED**

Run: `cargo test -p agent-core event_roundtrip -- TaskConfirmation`

Expected: compile failure because `TaskConfirmationOption`, `TaskConfirmationDecision`, and event variants do not exist.

- [ ] **Step 3: Implement minimal core contract**

Add `TaskConfirmationOption` in `events.rs`, two payload variants, `event_type()` arms, and facade DTO:

```rust
pub struct TaskConfirmationDecision {
    pub request_id: String,
    pub selected_option_ids: Vec<String>,
    pub custom_response: Option<String>,
}
```

Add `SessionFacade::decide_task_confirmation` default implementation returning `Ok(())` so existing test fakes compile, and re-export from `agent-core/src/lib.rs`.

- [ ] **Step 4: Run GREEN**

Run: `cargo test -p agent-core event_roundtrip event_coverage`

Expected: non-zero matching tests pass.

### Task 2: Runtime Pending Confirmation Tool

**Files:**

- Create: `crates/agent-runtime/src/task_confirmation.rs`
- Modify: `crates/agent-runtime/src/lib.rs`
- Modify: `crates/agent-runtime/src/agent_loop/mod.rs`
- Modify: `crates/agent-runtime/src/agent_loop/turn_context.rs`
- Modify: `crates/agent-runtime/src/agent_loop/tool_loop.rs`
- Modify: `crates/agent-runtime/src/facade_bootstrap.rs`
- Test: `crates/agent-runtime/src/task_confirmation_tests.rs`
- Test: `crates/agent-runtime/src/agent_loop/tool_loop_tests.rs`

- [ ] **Step 1: Write failing runtime tests**

Add a focused `request_task_confirmation_emits_event_and_waits_for_decision` test that creates a pending map, starts the request future, resolves with `TaskConfirmationDecision`, and asserts output text contains selected ids and custom text.

Add a tool-loop test that passes a `ToolCall { name: "task_confirmation.request", arguments: json!({ "prompt": "...", "options": [...] }) }` and asserts:

- `TaskConfirmationRequested` is emitted.
- after `resolve_task_confirmation`, `TaskConfirmationResolved` is emitted.
- `ToolLoopResult.tool_results[0]` includes the user response.

- [ ] **Step 2: Run RED**

Run: `cargo test -p agent-runtime task_confirmation`

Expected: compile failure on missing module and tool-loop dependency.

- [ ] **Step 3: Implement minimal pending map**

Implement `PendingTaskConfirmationsMap`, `request_task_confirmation`, `resolve_task_confirmation`, and session cancellation denial. Shape mirrors `permission.rs` but uses `TaskConfirmationDecision`.

- [ ] **Step 4: Wire tool definition and tool-loop intercept**

In `prepare_turn_context`, append a `ToolDefinition`:

```rust
ToolDefinition {
    name: "task_confirmation.request".into(),
    description: "Ask the user to clarify an ambiguous task using selectable options and optional free-form text.".into(),
    parameters: json_schema_for_prompt_options(),
}
```

In `execute_tool_calls`, special-case this tool before normal permission checks and invocation. Parse `prompt`, `options`, `allow_multiple`, and `allow_custom`; wait for the pending decision; return a structured text result to the model.

- [ ] **Step 5: Run GREEN**

Run: `cargo test -p agent-runtime task_confirmation tool_loop`

Expected: new tests pass with non-zero test counts.

### Task 3: GUI Chat Item

**Files:**

- Modify: `apps/agent-gui/src/types/trace.ts`
- Modify: `apps/agent-gui/src/types/chatStream.ts`
- Modify: `apps/agent-gui/src/stores/trace.ts`
- Modify: `apps/agent-gui/src/composables/useChatStream.ts`
- Create: `apps/agent-gui/src/components/chat/ChatTaskConfirmationItem.vue`
- Modify: `apps/agent-gui/src/components/ChatPanel.vue`
- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`
- Test: `apps/agent-gui/src/composables/useChatStream.test.ts`
- Test: `apps/agent-gui/src/components/chat/ChatTaskConfirmationItem.test.ts`

- [ ] **Step 1: Write failing GUI tests**

Add a `useChatStream` test that a pending `TaskConfirmationRequested` trace event becomes:

```ts
{ kind: "task_confirmation", id: "clarify-1", prompt, options, allowMultiple: true, allowCustom: true }
```

Add a component test that checks:

- Multiple mode renders checkboxes.
- Custom textarea renders when allowed.
- Submit invokes `resolve_task_confirmation` with `{ requestId, selectedOptionIds, customResponse }`.

- [ ] **Step 2: Run RED**

Run: `cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test -- ChatTaskConfirmationItem useChatStream`

Expected: missing component/types failures.

- [ ] **Step 3: Implement GUI stream and component**

Use `TraceEntryData.kind = "task_confirmation"` with `status: "pending"` for unresolved requests. `ChatTaskConfirmationItem.vue` owns local selection/custom state, validates submit when at least one selected option or non-empty custom text exists, and invokes `resolve_task_confirmation`.

- [ ] **Step 4: Run GREEN**

Run: `cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test -- ChatTaskConfirmationItem useChatStream`

Expected: new tests pass.

### Task 4: TUI Stream And Composer Interaction

**Files:**

- Modify: `crates/agent-tui/src/components/chat/stream_types.rs`
- Modify: `crates/agent-tui/src/components/chat/stream/mod.rs`
- Modify: `crates/agent-tui/src/components/chat/stream_render_items.rs`
- Modify: `crates/agent-tui/src/components/chat/stream_render.rs`
- Modify: `crates/agent-tui/src/app_state.rs`
- Modify: `crates/agent-tui/src/components/chat/input.rs`
- Modify: `crates/agent-tui/src/components/chat.rs`
- Modify: `crates/agent-tui/src/components/effects.rs`
- Modify: `crates/agent-tui/src/components/command_types.rs`
- Modify: `crates/agent-tui/src/runtime_dispatch/session/messages.rs`
- Modify: `crates/agent-tui/src/runtime_dispatch/session/mod.rs`
- Test: `crates/agent-tui/tests/chat_stream.rs`
- Test: `crates/agent-tui/tests/chat_render.rs`
- Test: `crates/agent-tui/src/components/chat/tests/permissions.rs`

- [ ] **Step 1: Write failing TUI tests**

Add reducer/render tests for a pending task confirmation card that includes prompt, `[ ]` options, and submit hint.

Add input tests that `ShowTaskConfirmationPrompt` puts `ChatPanel` into `TaskConfirmationWait`, digit keys toggle option selection, typed text becomes custom response, and Enter emits `Command::ResolveTaskConfirmation`.

- [ ] **Step 2: Run RED**

Run: `cargo test -p agent-tui task_confirmation`

Expected: compile failure on missing TUI variants.

- [ ] **Step 3: Implement TUI reducer/render/input**

Add `ChatStreamItem::TaskConfirmation`, render unresolved requests inline, add `InputState::TaskConfirmationWait`, and route `Command::ResolveTaskConfirmation` to `runtime.resolve_task_confirmation`.

- [ ] **Step 4: Run GREEN**

Run: `cargo test -p agent-tui task_confirmation chat_stream chat_render`

Expected: focused tests pass with non-zero counts.

### Task 5: IPC Types, Quality Gates, Dev App Verification

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands/session.rs`
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify: `apps/agent-gui/src-tauri/src/specta.rs`
- Modify: generated bindings via `just gen-types`

- [ ] **Step 1: Add IPC command and RED/compile check**

Add `resolve_task_confirmation` command and register it in Tauri invoke handler + Specta export.

- [ ] **Step 2: Generate types**

Run: `just gen-types`

Expected: generated GUI event/command types include new event payloads and command.

- [ ] **Step 3: Format and focused gates**

Run:

```bash
cargo fmt --all
cargo test -p agent-core event_roundtrip event_coverage
cargo test -p agent-runtime task_confirmation tool_loop
cargo test -p agent-tui task_confirmation chat_stream chat_render
cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test -- ChatTaskConfirmationItem useChatStream
cargo fmt --all --check
bun run format:check
bun run lint
```

- [ ] **Step 4: Dev App verification**

Run:

```bash
bun --filter agent-gui tauri dev --features pilot &
until tauri-pilot ping 2>/dev/null; do sleep 2; done
```

Manual/pilot scenario: start a fake-model session that emits `task_confirmation.request`, verify the card appears in chat, select options, add custom text, submit, verify the pending card resolves and no JS errors appear via `tauri-pilot logs --level error`.

- [ ] **Step 5: Commit/PR path**

If local gates and Dev App verification pass, commit with `feat: add structured task confirmations`, push, open PR, enable auto-merge, watch CI, then clean up per `kairox-dev-workflow`.

### Acceptance Checklist

- [x] Model has a documented structured tool for ambiguous task clarification.
- [x] Runtime pauses for a user confirmation decision and resumes with a tool result.
- [x] GUI renders pending confirmations inside the chat panel with checkbox/radio options, optional custom text, and submit.
- [x] TUI renders pending confirmations inline and lets the composer submit option/custom decisions.
- [x] Resolved confirmations disappear from GUI inline stream but remain traceable through events.
- [x] Focused Rust/Vitest tests, format, lint, generated bindings, and Dev App verification have evidence.

### Execution Notes

- Core RED confirmed by `cargo check -p agent-core --test event_roundtrip`; GREEN confirmed by `cargo test -p agent-core task_confirmation` and `cargo test -p agent-core --test event_coverage`.
- Runtime RED confirmed by `cargo test -p agent-runtime task_confirmation --no-run`; GREEN confirmed by `cargo test -p agent-runtime task_confirmation -- --nocapture`.
- GUI RED confirmed by focused Vitest failures for missing trace/chat/component handling; GREEN confirmed by `cd apps/agent-gui && bun run test -- src/composables/useChatStream.test.ts src/stores/trace.test.ts src/components/chat/ChatTaskConfirmationItem.test.ts`.
- TUI RED confirmed by `cargo test -p agent-tui task_confirmation --no-run`; GREEN confirmed by `cargo test -p agent-tui task_confirmation -- --nocapture`.
- Type generation completed with `just gen-types`; generated `commands.ts` includes `resolveTaskConfirmation`, and generated event bindings include `TaskConfirmationRequested` / `TaskConfirmationResolved`.
- Additional checks passed: `cargo fmt --all`, `cargo fmt --all --check`, `cargo check -p agent-gui-tauri`, `cargo test -p agent-core --test event_coverage`, `bunx oxfmt --check ...` for touched GUI files, `bun run lint:style`, and `git diff --check`.
- `bunx vue-tsc --noEmit` is blocked by existing repository-wide type errors unrelated to this change, including mount helper overloads, stale test fixtures, generated/session fixture mismatches, dependency d.ts issues, and existing `ContextUsage` type drift.
- Browser Vite smoke reached `Kairox Agent Workbench`, but plain browser mode produced expected Tauri `invoke` errors because `window.__TAURI__` is unavailable outside the desktop shell.
- Tauri pilot retry succeeded with `bun run tauri -- dev --features pilot`: `tauri-pilot --socket /tmp/tauri-pilot-dev.kairox.agent.dev1420.sock ping` connected to plugin/CLI `0.7.2`, `state` reported `Kairox Agent Workbench` at `http://localhost:1420/#/workbench`, the injected `TaskConfirmationRequested` rendered inside `[data-test="chat-task-confirmation-item"]`, selecting `task-confirmation-option-small` plus custom text enabled submit, clicking submit produced no captured error logs, and injecting `TaskConfirmationResolved` removed the pending card while the trace moved from Active to Done.
- PR worktree verification on `feat/task-confirmations` completed with `just gen-types`, `cargo fmt --all --check`, `cargo test -p agent-core -p agent-runtime -p agent-tui`, `cargo clippy -p agent-core -p agent-runtime -p agent-tui -p agent-gui-tauri --all-targets -- -D warnings`, focused GUI Vitest for `useChatStream`, `trace`, and `ChatTaskConfirmationItem`, `cd apps/agent-gui && bun run build`, `bun run format:check`, `bun run lint`, and `git diff --check`.
- PR worktree Dev App verification completed with Vite on `http://localhost:1420` plus `target/debug/agent-gui-tauri` built with `pilot`; `tauri-pilot` connected to plugin/CLI `0.7.2`, reported `Kairox Agent Workbench` at `http://localhost:1420/#/workbench`, rendered the injected task confirmation card in chat, enabled submit after selecting `small` and filling `Keep public API stable`, produced no captured error logs after submit, and removed the card after `TaskConfirmationResolved` while trace counts moved to Active 0 / Done 1. The initial `bun run tauri -- dev --features pilot` wrapper exited after spawning the binary in this worktree, so the verified path kept Vite and the GUI binary running explicitly.

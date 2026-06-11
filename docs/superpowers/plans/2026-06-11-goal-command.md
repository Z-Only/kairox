# Goal Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a user-facing goal command in GUI and TUI chat inputs.

**Architecture:** Reuse existing command palette and chat-send paths. GUI exposes `/goal` in the Vue command registry, then the Tauri chat command rewrites `/goal <text>` into goal-oriented model content while preserving the original command in `display_content`; TUI exposes `:goal` in its palette and rewrites at runtime dispatch before calling `SendMessageRequest`.

**Tech Stack:** Vue 3 + Vitest, Tauri Rust command tests, ratatui TUI command parser tests, Cargo test/clippy, bun lint/format.

---

## Touched Files

- Modify: `apps/agent-gui/src/composables/useCommandRegistry.ts`
- Modify: `apps/agent-gui/src/composables/useCommandRegistry.test.ts`
- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`
- Modify: `apps/agent-gui/src-tauri/src/commands/chat.rs`
- Modify: `crates/agent-tui/src/components/command_palette/registry.rs`
- Modify: `crates/agent-tui/src/components/command_palette/registry_tests.rs`
- Modify: `crates/agent-tui/src/components/command_palette/tests.rs`
- Modify: `crates/agent-tui/src/runtime_dispatch/session/messages.rs`
- Add: `crates/agent-tui/src/runtime_dispatch/session/messages_tests.rs`
- Modify: `crates/agent-tui/tests/app_logic_chat.rs`

## Forbidden Files

- Do not edit generated GUI bindings under `apps/agent-gui/src/generated/`; the `send_message` IPC signature stays unchanged.
- Do not change `agent-core` facade/event types or store migrations.
- Do not repurpose `/plan` or `/auto`.

## Acceptance Signals

- GUI command palette includes `/goal` as a session-active command and inserts `/goal `.
- GUI backend sends a goal-formatted model prompt while keeping `display_content` as the original `/goal ...` text.
- Empty `/goal` or `/goal   ` falls through as normal content, matching existing malformed command behavior.
- TUI palette includes `:goal <objective>` and prefills `:goal `.
- TUI runtime dispatch sends goal-formatted model content with original `:goal ...` display content.
- Existing `/plan`, `/auto`, `/model`, attachments, and queue behavior keep working.

### Task 1: GUI Command Registry

**Files:**

- Modify: `apps/agent-gui/src/composables/useCommandRegistry.test.ts`
- Modify: `apps/agent-gui/src/composables/useCommandRegistry.ts`
- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`

- [x] **Step 1: Write RED tests**

Add expectations that the builtin command order includes `goal`, it is filtered out without an active session, it localizes through `chat.commands.goal.description`, and it inserts `/goal `.

Run:

```bash
cd apps/agent-gui
bun run test src/composables/useCommandRegistry.test.ts
```

Expected: FAIL because `goal` is absent.

- [x] **Step 2: Implement GREEN**

Add a builtin command:

```ts
{
  id: "goal",
  label: "/goal",
  descriptionKey: "chat.commands.goal.description",
  context: "session-active",
  insertText: "/goal "
}
```

Add locale descriptions:

```json
"goal": { "description": "Set a concrete objective for the next agent turn" }
```

and Chinese equivalent:

```json
"goal": { "description": "为下一次代理回合设置明确目标" }
```

- [x] **Step 3: Verify GREEN**

Run:

```bash
cd apps/agent-gui
bun run test src/composables/useCommandRegistry.test.ts
```

Expected: PASS.

### Task 2: GUI Send Rewrite

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/commands/chat.rs`

- [x] **Step 1: Write RED tests**

Add unit tests for a helper that rewrites `/goal ship the feature` to model content that contains `# Goal` and `ship the feature`, and returns display content `/goal ship the feature`. Add a malformed `/goal` test that leaves content unchanged and display content `None`.

Run:

```bash
cargo test -p agent-gui-tauri chat_attachment_tests::goal
```

Expected: FAIL because the helper is absent.

- [x] **Step 2: Implement GREEN**

Add a small helper in `chat.rs`:

```rust
fn prepare_goal_message(content: String) -> (String, Option<String>) {
    let Some(goal) = content.strip_prefix("/goal ").map(str::trim).filter(|goal| !goal.is_empty()) else {
        return (content, None);
    };
    let model_content = format!(
        "# Goal\n\n{goal}\n\nWork toward this goal until it is complete. Track progress, verify concrete changes, and report blockers explicitly."
    );
    (model_content, Some(content))
}
```

Call it before attachment enrichment so the model-facing goal content is enriched when attachments exist, and preserve the original `/goal ...` text in `display_content`.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p agent-gui-tauri chat_attachment_tests::goal
```

Expected: PASS.

### Task 3: TUI Palette and Dispatch

**Files:**

- Modify: `crates/agent-tui/src/components/command_palette/registry.rs`
- Modify: `crates/agent-tui/src/components/command_palette/registry_tests.rs`
- Modify: `crates/agent-tui/src/components/command_palette/tests.rs`
- Modify: `crates/agent-tui/src/runtime_dispatch/session/messages.rs`
- Modify: `crates/agent-tui/tests/app_logic_chat.rs`

- [x] **Step 1: Write RED tests**

Add tests that `:goal` appears in the palette, `prefill_text(&PaletteAction::PrefillGoal) == Some(":goal ")`, selecting goal emits `CrossPanelEffect::PrefillChatInput(":goal ")`, and chat input `:goal fix tests` emits `Command::SendMessage { content: ":goal fix tests", .. }` for runtime dispatch to rewrite.

Run:

```bash
cargo test -p agent-tui command_palette::registry_tests::prefill_text_returns_some_for_goal app_logic_chat::colon_goal_input_sends_goal_command_text
```

Expected: FAIL because the action/parser support is absent.

- [x] **Step 2: Implement GREEN**

Add `PrefillGoal` to `PaletteAction`, add static entry:

```rust
PaletteEntry::static_entry(
    "goal",
    ":goal <objective>",
    "Send a concrete objective for the agent to pursue",
    PaletteAction::PrefillGoal,
)
```

Add `PrefillGoal => Some(":goal ")`.

Add runtime helper in `messages.rs` that turns `:goal fix tests` into the same model content as GUI and passes `display_content: Some(original)`.

- [x] **Step 3: Verify GREEN**

Run:

```bash
cargo test -p agent-tui goal
```

Expected: PASS for goal-focused TUI tests.

### Task 4: Quality Gates and Dev App Verification

**Files:**

- All changed files.

- [x] **Step 1: Focused tests**

```bash
cd apps/agent-gui
bun run test src/composables/useCommandRegistry.test.ts
cargo test -p agent-gui-tauri goal
cargo test -p agent-tui goal
```

- [x] **Step 2: Required gates**

```bash
cargo fmt --all --check
cargo clippy -p agent-gui-tauri --all-targets -- -D warnings
cargo clippy -p agent-tui --all-targets -- -D warnings
cargo test -p agent-gui-tauri
cargo test -p agent-tui
bun run format:check
bun run lint
```

- [x] **Step 3: Local Dev App verification with kairox-live**

Start Kairox GUI with pilot:

```bash
bun --filter agent-gui tauri dev --features pilot
```

Use `tauri-pilot`/`kairox-live` to verify:

- command palette opens after typing `/go`
- `/goal` item is visible
- selecting it inserts `/goal `
- sending `/goal test objective` produces a user message displaying `/goal test objective`
- no JS errors in `tauri-pilot logs --level error`

Cleanup:

```bash
lsof -nP -iTCP:1420 -sTCP:LISTEN | awk 'NR>1{print $2}' | xargs kill 2>/dev/null || true
```

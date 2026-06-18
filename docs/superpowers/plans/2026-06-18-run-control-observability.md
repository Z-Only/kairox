# Run Control Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an idle-only session send path for automation and reduce false tool failures from short agent-loop shell timeouts.

**Architecture:** Preserve the existing GUI follow-up queue behavior from the runtime actor. Add an explicit runtime/Tauri path that rejects active running turns for automation that must not enqueue stale continuations. Replace hard-coded agent-loop tool timeouts with one small policy function that gives shell commands a longer budget while keeping other tools unchanged.

**Tech Stack:** Rust `agent-runtime`, Tauri raw IPC command layer, focused Rust tests, Tauri Pilot verification.

---

### File Map

- Modify: `crates/agent-runtime/src/facade_session_ops.rs`
  - Add an idle-only preflight/send path that rejects `ExecutionState::Running` and `ExecutionState::Cancelling`.
- Modify: `crates/agent-runtime/src/facade_runtime/tests/send_message_tests.rs`
  - Add RED coverage that idle-only send returns `SessionBusy` while a turn is running.
- Modify: `crates/agent-runtime/src/agent_loop/tool_loop.rs`
  - Add `tool_invocation_timeout_ms` and use it for risk and execution invocations.
- Modify: `crates/agent-runtime/src/agent_loop/tool_loop_tests.rs`
  - Add RED coverage that `shell.exec` receives a longer timeout and non-shell tools keep 30s.
- Modify: `apps/agent-gui/src-tauri/src/commands/chat.rs`
  - Add `send_message_to_session_if_idle` command, a compile guard test, and an executable helper test for the idle IPC path.
- Modify: `apps/agent-gui/src-tauri/src/lib.rs`
  - Register the new raw Tauri command. This command is intentionally not exported through Specta typed bindings because it is for `tauri-pilot`/automation IPC, not Vue code.
- Modify ignored skill files in main checkout, not this worktree:
  - `.agents/skills/kairox-skill-proxy/SKILL.md`
  - `.agents/skills/kairox-evaluate-kairox/SKILL.md`
  - `.agents/skills/kairox-dev-workflow/SKILL.md`

### Task 1: Idle-only Send Path

**Files:**

- Modify: `crates/agent-runtime/src/facade_session_ops.rs`
- Test: `crates/agent-runtime/src/facade_runtime/tests/send_message_tests.rs`
- Modify: `apps/agent-gui/src-tauri/src/commands/chat.rs`
- Modify command registration files listed above.

- [x] **Step 1: Write the failing runtime test**

Add a test next to `send_message_strict_queues_same_session_turn_when_actor_turn_running`:

```rust
#[tokio::test]
async fn send_message_if_idle_rejects_running_session() {
    // Start a blocking turn, then call the new idle-only send method.
    // Expected: CoreError::SessionBusy with a reason containing "running".
}
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p agent-runtime send_message_if_idle_rejects_running_session -- --nocapture
```

Expected: compile failure or missing method failure before implementation.

- [x] **Step 3: Implement minimal runtime path**

Add a method such as `send_message_if_idle` plus an internal `ensure_session_idle_for_send` guard. Preserve `send_message_queued` behavior.

- [x] **Step 4: Add Tauri command**

Add `send_message_to_session_if_idle` that builds the request and calls the runtime idle-only method. Register it in the Tauri handler list; do not export it through Specta unless Vue typed callers are added.

- [x] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p agent-runtime send_message_if_idle_rejects_running_session -- --nocapture
cargo test -p agent-runtime send_message_strict_queues_same_session_turn_when_actor_turn_running -- --nocapture
```

Expected: idle-only rejects running; queued strict behavior still passes.

### Task 2: Agent-loop Shell Timeout Policy

**Files:**

- Modify: `crates/agent-runtime/src/agent_loop/tool_loop.rs`
- Test: `crates/agent-runtime/src/agent_loop/tool_loop_tests.rs`

- [x] **Step 1: Write failing timeout tests**

Create a recording fake tool that captures `ToolInvocation.timeout_ms`. Assert:

```rust
shell.exec -> 300_000
greet -> 30_000
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test -p agent-runtime agent_loop::tool_loop_tests::shell_exec_uses_longer_agent_loop_timeout --lib -- --nocapture
```

Expected: shell timeout is still 30_000.

- [x] **Step 3: Implement timeout policy**

Add constants:

```rust
const DEFAULT_TOOL_TIMEOUT_MS: u64 = 30_000;
const LONG_RUNNING_SHELL_TIMEOUT_MS: u64 = 300_000;
```

Use a helper:

```rust
fn tool_invocation_timeout_ms(tool_id: &str) -> u64 {
    if tool_id == "shell.exec" { LONG_RUNNING_SHELL_TIMEOUT_MS } else { DEFAULT_TOOL_TIMEOUT_MS }
}
```

Use it for both risk and execution `ToolInvocation`.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p agent-runtime shell_exec_uses_longer_agent_loop_timeout --lib -- --nocapture
cargo test -p agent-runtime non_shell_tools_keep_default_agent_loop_timeout --lib -- --nocapture
```

### Task 3: Raw IPC Decision and Skill Updates

**Files:**

- No generated file changes: `send_message_to_session_if_idle` is raw IPC only.
- Skill docs in main checkout `.agents/skills/**`

- [x] **Step 1: Validate generated binding scope**

Attempt `just gen-types` and avoid committing unrelated generated drift. The command is intentionally not exported through Specta typed bindings because it is for `tauri-pilot`/automation IPC, not Vue code.

- [x] **Step 2: Update SKILL docs**

Update proxy/evaluation workflow to prefer `send_message_to_session_if_idle` for continuations and to treat IPC timeouts as unknown enqueue state that must be checked via events before retrying. Update dev workflow with long-running command timeout and generator fallback guidance.

- [x] **Step 3: Validate skill shape**

Run simple frontmatter checks:

```bash
sed -n '1,20p' .agents/skills/kairox-skill-proxy/SKILL.md
sed -n '1,20p' .agents/skills/kairox-evaluate-kairox/SKILL.md
sed -n '1,20p' .agents/skills/kairox-dev-workflow/SKILL.md
```

### Task 4: Final Verification

- [x] Run `cargo fmt --all`
- [x] Run `cargo fmt --all --check`
- [x] Run `cargo test -p agent-runtime send_message`
- [x] Run `cargo test -p agent-runtime tool_loop --lib`
- [x] Run `cargo test -p agent-gui-tauri send_message_to_session_if_idle_command_is_compiled`
- [x] Run `just gen-types` or report exact generator blocker.
- [x] Dev App verification with Tauri Pilot raw IPC.

## Execution Log

- RED observed: `cargo test -p agent-runtime send_message_if_idle_rejects_running_session -- --nocapture` failed before implementation because `send_message_if_idle` did not exist.
- RED observed: `cargo test -p agent-runtime shell_exec_uses_longer_agent_loop_timeout --lib -- --nocapture` failed with `left: [30000]`, `right: [300000]`.
- GREEN verified: `cargo test -p agent-runtime send_message_if_idle_rejects_running_session -- --nocapture`.
- Regression verified: `cargo test -p agent-runtime send_message_strict_queues_same_session_turn_when_actor_turn_running -- --nocapture`.
- GREEN verified: `cargo test -p agent-runtime shell_exec_uses_longer_agent_loop_timeout --lib -- --nocapture`.
- GREEN verified: `cargo test -p agent-runtime non_shell_tools_keep_default_agent_loop_timeout --lib -- --nocapture`.
- Tauri command verified: `cargo test -p agent-gui-tauri send_message_to_session_if_idle_command_is_compiled -- --nocapture`.
- Format verified: `cargo fmt --all`; `cargo fmt --all --check`.
- Focused suites verified: `cargo test -p agent-runtime send_message -- --nocapture`; `cargo test -p agent-runtime tool_loop --lib -- --nocapture`.
- Lint verified: `cargo clippy -p agent-runtime --all-targets -- -D warnings`.
- Generator blocker: `just gen-types` failed first at `export-specta` with signal 9. A retry wrote `commands.ts` but then failed at `export-events` with signal 9; the generated `commands.ts` diff also shrank from 19096 lines to 1256 lines, so it was restored to avoid unrelated generator drift. The new idle-only command is now kept as raw Tauri IPC and is not exported through Specta, so no generated files are included in this lane.
- Full runtime suite verified: `cargo test -p agent-runtime`.
- Full Tauri backend suite verified: `cargo test -p agent-gui-tauri`.
- Workspace lint verified: `bun run lint` passed; existing non-fatal warning remains in `apps/agent-gui/src/stores/modelProfiles.test.ts:269:13`.
- Workspace format verified: `bun run format:web`; `bun run format:check`.
- Dev App verified with Tauri Pilot raw IPC:
  - Started Vite at `http://localhost:1420/`.
  - Started the worktree `agent-gui-tauri` debug binary with `pilot` feature and socket `/tmp/tauri-pilot-dev.kairox.agent.dev1420.sock`.
  - `tauri-pilot ping` passed and `snapshot -i` showed the Kairox workbench UI with Fake model selected.
  - `ipc start_session --args '{"profile":"fake"}'` returned `ses_e9a4166744d247a19c0ce8e49ba96620`.
  - `ipc send_message_to_session_if_idle --args '{"sessionId":"ses_e9a4166744d247a19c0ce8e49ba96620","content":"Dev App verification for idle-only IPC path","attachments":[]}'` returned success.
  - SQLite event verification showed `UserMessageAdded` with the test content and `AssistantMessageCompleted` with `Hello from the Kairox fake provider!`.
  - `tauri-pilot logs --level error` returned `No logs captured`.
- CI coverage fix:
  - Initial PR run failed `Coverage (Rust)` because `T2 Tauri IPC lines` was `40.92% < 41%`.
  - Added `send_message_to_session_if_idle_inner_runs_idle_turn` to execute the new raw IPC path with an in-memory runtime and Fake model.
  - Re-verified `cargo test -p agent-gui-tauri`, `cargo clippy -p agent-gui-tauri --all-targets -- -D warnings`, and `bun run format:check`.
  - Local `bun run coverage:rust` could not produce a comparable coverage result on this macOS host because `aws-lc-sys` stopped during the instrumented build with its local `cc` compiler bug guard.

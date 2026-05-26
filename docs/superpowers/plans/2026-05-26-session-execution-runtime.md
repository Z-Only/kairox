# Session Execution Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Kairox toward an actor/session execution runtime where each session owns a command mailbox and execution lifecycle, enabling future pause/resume, retry/cancel, re-planning, and safer DAG scheduling.

**Architecture:** Add a new `agent-runtime::execution_runtime` module. PR-1 introduces a session actor, runtime manager, command/result types, and a `TurnExecutor` trait while leaving `LocalRuntime::send_message()` behavior unchanged. Later PRs wire `LocalRuntime` through the actor, move task control into the mailbox, then upgrade DAG scheduling.

**Tech Stack:** Rust, Tokio `mpsc`/`oneshot`, `tokio_util::sync::CancellationToken`, `async_trait`, existing `agent-core` request/result types.

---

## PR-1: Session Actor Runtime Base

### Task 1: Define Runtime Types

**Files:**

- Create: `crates/agent-runtime/src/execution_runtime/mod.rs`
- Create: `crates/agent-runtime/src/execution_runtime/types.rs`
- Modify: `crates/agent-runtime/src/lib.rs`

- [ ] **Step 1: Write the failing type-level tests**

Create tests in `crates/agent-runtime/src/execution_runtime/tests.rs` that import:

```rust
use crate::execution_runtime::{
    ExecutionCommand, ExecutionState, SessionExecutionRuntime, TurnExecutor,
};
```

Expected before implementation: compile fails because the module and types do not exist.

- [ ] **Step 2: Implement exported types**

Add:

```rust
pub enum ExecutionState {
    Idle,
    Running { turn_id: String },
    Cancelling { turn_id: String },
    Stopped,
}

pub enum ExecutionCommand {
    RunTurn(agent_core::SendMessageRequest),
    Cancel { reason: String },
    RetryTask { task_id: agent_core::TaskId },
    CancelTask { task_id: agent_core::TaskId },
    Shutdown,
}
```

Add a `TurnExecutor` trait that accepts a `SendMessageRequest` and `CancellationToken`.

- [ ] **Step 3: Export the module**

Update `crates/agent-runtime/src/lib.rs`:

```rust
pub mod execution_runtime;
pub use execution_runtime::{SessionExecutionRuntime, TurnExecutor};
```

### Task 2: Implement Session Actor Lifecycle

**Files:**

- Create: `crates/agent-runtime/src/execution_runtime/session_actor.rs`
- Modify: `crates/agent-runtime/src/execution_runtime/mod.rs`
- Test: `crates/agent-runtime/src/execution_runtime/tests.rs`

- [ ] **Step 1: Write failing actor tests**

Cover these behaviors:

- `run_turn_completes_and_returns_to_idle`
- `run_turn_rejects_when_session_is_busy`
- `cancel_running_turn_triggers_cancellation_token`
- `shutdown_stops_actor`

- [ ] **Step 2: Run tests and confirm RED**

Run:

```bash
cargo test -p agent-runtime execution_runtime
```

Expected: FAIL before implementation because actor behavior is missing.

- [ ] **Step 3: Implement minimal actor**

Use one Tokio task per session actor. Commands enter through an `mpsc::Sender`. Requests that need a caller result carry an `oneshot::Sender`.

The actor must:

- transition `Idle -> Running -> Idle` around `TurnExecutor::execute_turn`;
- return `SessionBusy` when a second run arrives during `Running` or `Cancelling`;
- cancel the current `CancellationToken` on `Cancel`;
- transition to `Stopped` on `Shutdown`.

- [ ] **Step 4: Verify GREEN**

Run:

```bash
cargo test -p agent-runtime execution_runtime
```

Expected: PASS.

### Task 3: Implement Runtime Manager

**Files:**

- Create: `crates/agent-runtime/src/execution_runtime/runtime.rs`
- Modify: `crates/agent-runtime/src/execution_runtime/mod.rs`
- Test: `crates/agent-runtime/src/execution_runtime/tests.rs`

- [ ] **Step 1: Write failing manager tests**

Cover:

- same `session_id` reuses one actor;
- different `session_id` can run concurrently;
- `cancel_session()` forwards to the correct actor.

- [ ] **Step 2: Implement `SessionExecutionRuntime<E>`**

The manager owns:

```rust
actors: Arc<Mutex<HashMap<String, SessionActorHandle>>>
```

It exposes:

- `new(executor: Arc<E>)`
- `run_turn(request) -> agent_core::Result<()>`
- `cancel_session(session_id, reason) -> agent_core::Result<()>`
- `shutdown_session(session_id) -> agent_core::Result<()>`

- [ ] **Step 3: Verify manager tests**

Run:

```bash
cargo test -p agent-runtime execution_runtime
```

Expected: PASS.

## PR-2: Wire LocalRuntime Through The Actor

- Extract the existing `SessionFacade::send_message()` execution path into a `LocalRuntimeTurnExecutor`.
- Add `session_execution_runtime` to `LocalRuntime`.
- Route `send_message()` through `SessionExecutionRuntime::run_turn()`.
- Preserve current compaction busy checks and `/plan` routing.
- Add tests that a second concurrent `send_message()` returns `SessionBusy`.

## PR-3: Task Commands Through The Actor

- Route `retry_task()` and `cancel_task()` through actor commands.
- Keep current facade signatures stable.
- Ensure DAG graph mutation is serialized per session.
- Add tests for retry/cancel during active and idle execution states.

## PR-4: DAG Scheduler Upgrade

- Add `ExecutionBatch` and readiness diagnostics to `TaskGraph`.
- Make `DagExecutor` consume batches from the session actor.
- Add bounded parallel execution only after permission/tool state isolation is explicit.
- Add tests for batch ordering, max concurrency, blocked diagnostics, and cancellation propagation.

## Verification Gates

Run for each PR:

```bash
cargo test -p agent-runtime execution_runtime
cargo test -p agent-runtime dag_executor
cargo check --workspace
bun run format:check
bun run lint
```

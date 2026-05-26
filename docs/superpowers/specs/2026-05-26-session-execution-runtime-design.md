# Session Execution Runtime Design

> Date: 2026-05-26
> Status: Approved direction for multi-PR implementation
> Scope: `agent-runtime` execution orchestration, session-scoped actors, DAG executor integration path

## 1. Background

Kairox currently has two execution paths:

- `SingleStep`: `LocalRuntime::send_message()` calls the agent loop directly.
- `DagExecution`: `/plan` routes into `DagExecutor`, which owns a `TaskGraph` and scheduling loop.

This is enough for an early DAG model, but it does not provide a durable execution boundary for future features such as pause/resume, per-session command queues, dynamic re-planning, background task control, structured cancellation, or richer Codex/Claude-Code-style task orchestration. The current `DagExecutor` also claims configurable concurrency, but its scheduling loop still runs ready tasks sequentially.

## 2. Goal

Move the scheduling core toward an actor/session execution runtime:

- Each session gets a session-scoped execution actor.
- User turns, task retry, task cancel, pause/resume, and future re-plan commands enter through that actor mailbox.
- The actor owns session execution state and cancellation tokens.
- `SingleStep` and `DagExecution` become execution strategies behind the same session runtime boundary.
- Existing facade and GUI/TUI task APIs stay compatible while the internals migrate.

This is a multi-PR architecture change. The first PR should create the runtime boundary and prove the lifecycle model without replacing every execution path at once.

## 3. Non-Goals For PR-1

- Do not replace `DagExecutor` scheduling in one large patch.
- Do not change GUI/IPC payloads.
- Do not add a visual DAG editor.
- Do not persist full task graphs to SQLite yet.
- Do not implement true parallel worker execution until per-task policy/tool state is isolated from shared mutable runtime state.

## 4. Architecture

### 4.1 Session Execution Actor

Each active session has one actor:

```text
LocalRuntime facade
  -> SessionExecutionRuntime
      -> SessionExecutionActor(session_id)
          mailbox:
            RunTurn(request)
            Cancel(reason)
            RetryTask(task_id)
            CancelTask(task_id)
            Shutdown
```

The actor serializes commands for a session. A second `RunTurn` while a turn is running returns `SessionBusy` instead of starting overlapping model/tool work.

### 4.2 Turn Executor

The actor delegates actual work to a `TurnExecutor` trait:

```rust
#[async_trait]
pub trait TurnExecutor: Send + Sync {
    async fn execute_turn(
        &self,
        request: SendMessageRequest,
        cancellation: CancellationToken,
    ) -> agent_core::Result<()>;
}
```

PR-1 provides this trait and tests it with fakes. A later PR adapts `LocalRuntime` so the trait chooses `SingleStep` vs `DagExecution` and reuses existing code.

### 4.3 State Model

The actor tracks:

- `Idle`
- `Running { turn_id }`
- `Cancelling { turn_id }`
- `Stopped`

The actor also emits in-memory lifecycle outcomes to callers:

- `Accepted`
- `RejectedBusy`
- `Cancelled`
- `Completed`
- `Failed`
- `Stopped`

Domain events remain unchanged in PR-1. Once `LocalRuntime` is wired through the actor, existing `DomainEvent` writes still come from the underlying agent loop or DAG executor.

### 4.4 Relationship To DAG

The actor is above the DAG executor. It does not replace `TaskGraph`; it gives task execution a session-scoped control plane. Future PRs can then:

- Make `DagExecutor` a strategy called by `TurnExecutor`.
- Move retry/cancel task commands through the actor mailbox.
- Add re-plan commands that mutate the task graph through one serialized owner.
- Add bounded worker parallelism after task-local policy state is isolated.

## 5. Multi-PR Plan

### PR-1: Session Actor Runtime Base

- Add `agent-runtime::execution_runtime`.
- Add `TurnExecutor`, `SessionExecutionRuntime`, session actor command/state types.
- Prove lifecycle behavior with tests: serialized user turns, busy rejection, cancellation token propagation, actor shutdown.
- Keep `LocalRuntime::send_message()` unchanged.

### PR-2: Wire LocalRuntime Through The Actor

- Extract current send-message execution body into a `LocalRuntimeTurnExecutor`.
- Route `SessionFacade::send_message()` through `SessionExecutionRuntime`.
- Preserve `SessionBusy` behavior during compaction and running turns.
- Keep `/plan` DAG selection behavior unchanged behind the new boundary.

### PR-3: Move Task Control Into The Actor

- Route retry/cancel task commands through the session actor.
- Ensure DAG mutations happen through one serialized owner.
- Keep current facade methods stable.

### PR-4: DAG Scheduler Upgrade

- Introduce execution batches and explicit readiness diagnostics.
- Add bounded worker concurrency only after shared permission/tool state is safe.
- Add dynamic re-planning hooks as actor commands.

## 6. Risks

- Shared mutable `PermissionEngine` cannot be used naively by parallel workers because agent settings can temporarily override approval/sandbox policy.
- Task graph state is still in memory. Actor ownership reduces concurrent mutation risk but does not solve persistence by itself.
- A runtime actor must not swallow existing domain events; event emission remains inside the existing execution strategies until wiring is complete.

## 7. Acceptance

The objective is complete only when `LocalRuntime` routes session execution through actors, DAG task control goes through the session mailbox, and the old direct scheduling path is no longer the primary control plane. PR-1 is only the base slice.

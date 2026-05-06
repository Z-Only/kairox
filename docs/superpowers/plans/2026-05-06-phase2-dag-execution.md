# Phase 2: DAG Execution + Agent Strategy — Implementation Plan

**Date:** 2026-05-06
**Depends on:** Phase 1 (module split) — COMPLETE
**Spec:** `docs/superpowers/specs/2026-05-05-agent-loop-deep-implementation-design.md`

## Overview

Implement `DagExecutor` + `AgentStrategy` trait, enabling Planner → Worker → Reviewer orchestration
with parallel execution, failure cascade, and retry/skip recovery. Execution is opt-in via `/plan` prefix.

## Task Breakdown

### T1: Extend TaskState and AgentTask in agent-core

**Files:** `crates/agent-core/src/task_types.rs`

- Add `Ready` and `Skipped` variants to `TaskState` (between `Pending` and `Running` in order)
- Add fields to `AgentTask` (used in `task_graph.rs`, but the types live in core):
  - `retry_count: usize`
  - `max_retries: usize`
  - `assigned_agent_id: Option<AgentId>`
  - `failure_reason: Option<TaskFailureReason>`
- Add `TaskFailureReason` enum to `task_types.rs`:
  ```rust
  pub enum TaskFailureReason {
      ModelError { retries: usize },
      ToolExhausted { tool_id: String, attempts: usize, last_error: String },
      PermissionDenied { tool_id: String },
      Cancelled,
      MaxIterations,
  }
  ```
- Add `FailurePolicy` enum:
  ```rust
  pub enum FailurePolicy {
      BlockDependents,
      AllowOrphans,
      FailFast,
  }
  ```
- Add `RetryConfig` struct:
  ```rust
  pub struct RetryConfig {
      pub max_model_retries: usize,   // default 3
      pub max_tool_retries: usize,    // default 2
      pub backoff: BackoffStrategy,   // default ExponentialJitter
  }
  pub enum BackoffStrategy {
      Fixed { delay_ms: u64 },
      ExponentialJitter { base_ms: u64, max_ms: u64 },
  }
  ```
- Update `TaskSnapshot` in `facade.rs` to include `assigned_agent_id` and `retry_count`
- Update specta registration if needed

**Tests:** Unit tests for serialization of new variants, default values for RetryConfig

### T2: Add new EventPayload variants in agent-core

**Files:** `crates/agent-core/src/events.rs`

Add to `EventPayload`:

- `TaskDecomposed { parent_task_id: TaskId, sub_task_ids: Vec<TaskId> }`
- `TaskBlocked { task_id: TaskId, blocking_task_id: TaskId, reason: String }`
- `AgentSpawned { agent_id: String, role: String, task_id: TaskId }`
- `AgentIdle { agent_id: String }`
- `TaskRetried { task_id: TaskId, attempt: usize }`

Update `event_type()` match arm.

**Tests:** Serialization round-trip tests for each new variant

### T3: Extend TaskGraph with new state machine methods

**Files:** `crates/agent-runtime/src/task_graph.rs`

- `add_task_with_config()` — allows setting `max_retries`, `assigned_agent_id` at creation
- `mark_blocked(&mut self, id: &TaskId, reason: String)` — Pending/Ready → Blocked
- `mark_skipped(&mut self, id: &TaskId)` — any non-completed → Skipped
- `reset_to_pending(&mut self, id: &TaskId)` — Failed/Blocked → Pending (for retry)
- `mark_ready(&mut self, id: &TaskId)` — Pending → Ready
- `find_blocked_dependents(&self, id: &TaskId) -> Vec<TaskId>` — transitive blocked dependents
- `find_direct_dependents(&self, id: &TaskId) -> Vec<TaskId>` — immediate children
- `get_task(&self, id: &TaskId) -> Option<&AgentTask>`
- `is_finished(&self) -> bool` — all tasks in terminal state (Completed/Failed/Skipped/Cancelled)
- `mark_cancelled(&mut self, id: &TaskId)` — any → Cancelled
- Update `ready_tasks()` to also include `Ready` state tasks (not just Pending with deps met)
- Update `add_task` to accept the extended `AgentTask` fields (retry_count defaults to 0, max_retries defaults to 2)

Update `snapshot()` to include the new `AgentTask` fields.

**Tests:** 12+ unit tests covering the full state machine transitions, cascade, is_finished, etc.

### T4: Define AgentStrategy trait and types

**Files:** `crates/agent-runtime/src/agents.rs`

Replace the empty struct agents with the `AgentStrategy` trait system:

```rust
#[async_trait]
pub trait AgentStrategy: Send + Sync {
    fn role(&self) -> AgentRole;
    async fn build_context(
        &self,
        task: &AgentTask,
        graph: &TaskGraph,
        session_events: &[DomainEvent],
    ) -> Vec<ModelMessage>;
    async fn decide(
        &self,
        ctx: &StepContext,
        messages: Vec<ModelMessage>,
    ) -> AgentDecision;
    async fn process_tool_result(
        &self,
        tool_call: &ToolCall,
        result: &str,
        iteration: usize,
    ) -> ToolResultAction;
}
```

Define supporting types:

- `StepContext` (move from design spec)
- `StepOutcome` (move from design spec)
- `AgentDecision` enum
- `ToolResultAction` enum
- `SubTaskDef` struct

Keep `ReviewerFinding` and `ReviewerAgent::review_diff` for backward compat.

**Tests:** Unit tests for type construction, enum variants

### T5: Implement PlannerStrategy

**Files:** `crates/agent-runtime/src/agents/planner.rs` (new)

- Implements `AgentStrategy` for `PlannerStrategy`
- `build_context()` — constructs system prompt instructing JSON decomposition output
- `decide()` — parses model response:
  - If JSON with `sub_tasks` → `AgentDecision::Decompose { sub_tasks }`
  - If plain text → `AgentDecision::Respond(text)` (simple question, no decomposition needed)
- `process_tool_result()` — returns `Continue` (planner doesn't call tools)

**Tests:** 2+ tests with `FakeModelClient` returning JSON decomposition, plain text response

### T6: Implement WorkerStrategy

**Files:** `crates/agent-runtime/src/agents/worker.rs` (new)

- Implements `AgentStrategy` for `WorkerStrategy`
- `build_context()` — includes task description + dependency outputs
- `decide()` — returns `RequestModel { tools }` (worker can call tools)
- `process_tool_result()` — returns `Continue` or `Retry { max_retries }` on tool exhaustion

**Tests:** 2+ tests

### T7: Implement ReviewerStrategy

**Files:** `crates/agent-runtime/src/agents/reviewer.rs` (new)

- Implements `AgentStrategy` for `ReviewerStrategy`
- `build_context()` — includes original task + worker output
- `decide()` — returns `ReviewComplete { approved, findings }`
- `process_tool_result()` — returns `Continue` (reviewer doesn't call tools)

**Tests:** 2+ tests

### T8: Implement DagExecutor

**Files:** `crates/agent-runtime/src/dag_executor.rs`

The main executor with:

- `new()` constructor taking store, model, strategies, config
- `is_available()` — returns true when at least a PlannerStrategy is registered
- `execute()` — the main entry point:
  1. Create root task in TaskGraph
  2. Call PlannerStrategy.decide() to decompose
  3. If Decompose → populate TaskGraph with sub-tasks, emit TaskDecomposed
  4. Scheduling loop:
     - `ready_tasks()` → spawn in `JoinSet` bounded by `Semaphore`
     - Each task: assign AgentStrategy, call build_context + decide
     - On task completion → `mark_completed()`, check for newly ready tasks
     - On task failure → `mark_failed()` + cascade `BlockDependents` (default)
     - On retry → `reset_to_pending()`, emit `TaskRetried`
     - Loop until `is_finished()`
  5. Call ReviewerStrategy.review() on completed worker outputs
  6. Return `ExecutionResult`

```rust
pub struct ExecutionResult {
    pub graph: TaskGraph,
    pub total_tasks: usize,
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
}
```

- `retry_task()` — public method for UI-driven retry
- `cancel_task()` — public method for UI-driven cancel
- `get_agent_status()` — returns active agent info

**Tests:** 9+ integration tests covering:

- Linear DAG (A → B → C)
- Parallel DAG (A → [B, C] → D)
- Failure cascade (B fails, D gets blocked)
- Skip failed task unblocks dependents
- Retry task resets to pending and re-executes
- Cancel task
- Concurrency limit (only N tasks running at once)
- Single-step mode still works (backward compat)
- `/plan` prefix triggers DAG mode

### T9: Wire DagExecutor into LocalRuntime

**Files:** `crates/agent-runtime/src/facade_runtime.rs`, `crates/agent-runtime/src/agent_loop.rs`

- Add `ExecutionMode` enum to `facade_runtime.rs`
- Add `dag_executor` field to `LocalRuntime` (optional, constructed with strategies)
- Add `with_dag_executor()` builder method
- In `send_message()`, check `execution_mode()`:
  - `SingleStep` → current `run_agent_loop()` behavior
  - `DagExecution` → `dag_executor.execute()`
- Update `get_task_graph()` to also work with DAG execution results
- Emit `AgentSpawned`, `AgentIdle`, `TaskBlocked`, `TaskRetried` events from DagExecutor

### T10: Add new Tauri commands and AppFacade methods

**Files:** `crates/agent-core/src/facade.rs`, `crates/agent-runtime/src/facade_runtime.rs`

Add to `AppFacade`:

- `async fn retry_task(&self, session_id: SessionId, task_id: TaskId) -> Result<()>`
- `async fn cancel_task(&self, session_id: SessionId, task_id: TaskId) -> Result<()>`
- `async fn get_agent_status(&self, session_id: SessionId) -> Result<Vec<AgentStatus>>`

Add `AgentStatus` type:

```rust
pub struct AgentStatus {
    pub agent_id: String,
    pub role: AgentRole,
    pub task_id: Option<TaskId>,
    pub status: String,  // "idle" | "running" | "completed" | "failed"
}
```

Implement in `LocalRuntime` by delegating to DagExecutor or task_graphs.

### T11: Extend FakeModelClient for role-based responses

**Files:** `crates/agent-models/src/fake.rs`

Add `role_responses` support:

```rust
pub struct FakeModelClient {
    tokens: Vec<String>,
    include_tool_call: bool,
    role_responses: HashMap<String, Vec<String>>,  // role → responses
}
```

When `role_responses` is set, check the model request's system prompt or messages
for role hints and return matching responses. This enables:

- Planner → returns sub-task JSON
- Worker → returns tool calls then completion
- Reviewer → returns approval/rejection

### T12: Write comprehensive tests

- **T12a:** TaskGraph state machine tests in `task_graph.rs` (12+)
- **T12b:** AgentStrategy unit tests in `agents/` modules (6+)
- **T12c:** DagExecutor integration tests in `tests/dag_executor.rs` (9+)
- **T12d:** ExecutionMode switching tests in `tests/execution_mode.rs` (2+)
- **T12e:** Run full existing test suite — zero regressions

### T13: Run `just gen-types` and update TypeScript bindings

After adding new EventPayload variants and AppFacade methods:

- Run `just gen-types`
- Verify generated files have no uncommitted changes
- Update `TaskSnapshot` TypeScript type if fields added

## Dependency Graph

```
T1 (core types) ──→ T2 (events) ──→ T3 (TaskGraph extensions)
                                        │
T4 (AgentStrategy trait) ──────────────→│
    │                                   │
    ├──→ T5 (PlannerStrategy)           │
    ├──→ T6 (WorkerStrategy) ──────────→│
    └──→ T7 (ReviewerStrategy) ─────────┤
                                         │
                    T8 (DagExecutor) ←───┘
                         │
                    T9 (wire into LocalRuntime)
                         │
                    T10 (AppFacade + Tauri commands)
                         │
                    T11 (FakeModelClient extension)
                         │
                    T12 (comprehensive tests)
                         │
                    T13 (type generation)
```

## Execution Order

1. **T1** → **T2** (core types and events, no behavior change)
2. **T3** (TaskGraph extensions, backward compatible)
3. **T4** (AgentStrategy trait definition)
4. **T5, T6, T7** in parallel (three strategy implementations)
5. **T11** (FakeModelClient extension, needed for T8 tests)
6. **T8** (DagExecutor implementation)
7. **T9** (wire into LocalRuntime)
8. **T10** (AppFacade new methods)
9. **T12** (comprehensive tests)
10. **T13** (type generation)

## Acceptance Criteria

- [ ] All existing tests pass (zero regressions)
- [ ] TaskGraph supports full state machine: Pending→Ready→Running→Completed/Failed/Blocked/Skipped/Cancelled
- [ ] AgentStrategy trait implemented with PlannerStrategy, WorkerStrategy, ReviewerStrategy
- [ ] DagExecutor handles linear, parallel, diamond DAGs
- [ ] Failure cascade (BlockDependents) works correctly
- [ ] Retry and skip task recovery works
- [ ] `/plan` prefix triggers DAG execution mode
- [ ] SingleStep mode unchanged (backward compatible)
- [ ] New event types emitted: TaskDecomposed, TaskBlocked, AgentSpawned, AgentIdle, TaskRetried
- [ ] New AppFacade methods: retry_task, cancel_task, get_agent_status
- [ ] 29+ new tests added (12 TaskGraph + 6 AgentStrategy + 9 DagExecutor + 2 ExecutionMode)
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes

# Agent Loop Deep Implementation — Design Spec

**Date:** 2026-05-05
**Status:** Approved
**Scope:** Refactor `facade_runtime.rs`, implement DAG-driven multi-step agent execution, evolve GUI for multi-agent attribution and task interaction.

---

## Problem

Kairox has a solid event-sourced architecture with trait-based crate boundaries, but the core agent loop has critical limitations:

1. **`facade_runtime.rs` is 1639 lines** — session management, agent loop, permission handling, memory processing, event emission, tool/MCP registration all in one file. It's difficult to modify without regressions.

2. **No real multi-agent execution** — `PlannerAgent`, `WorkerAgent`, `ReviewerAgent` are empty structs. The agent loop is a single synchronous LLM cycle: user message → model response → tool calls → repeat. Task decomposition, parallel execution, and result review don't exist.

3. **TaskGraph is display-only** — the data structure has `add_task`/`ready_tasks`/`mark_completed`, but the runtime never uses it for scheduling. It's populated for UI display, not execution.

4. **No error recovery** — tool call failures are reported to the LLM but there's no retry policy, no cascade blocking, no task retry, no partial DAG recovery.

5. **GUI cannot attribute work to agents** — `source_agent_id` exists on `DomainEvent` but is never read by the frontend. `isStreaming` is a single boolean, `token_stream` is a single string. Multi-agent concurrent streaming would overwrite itself.

## Goal

Transform the agent loop from a simple single-turn cycle into a DAG-driven multi-agent orchestration system, in three phases:

- **Phase 1 (Refactor)**: Split `facade_runtime.rs` into focused modules with zero behavior change.
- **Phase 2 (DAG Execution)**: Implement `DagExecutor` + `AgentStrategy` trait, enabling Planner → Worker → Reviewer orchestration with parallel execution and error recovery.
- **Phase 3 (GUI Evolution)**: Agent attribution, N-level task tree, permission queue, task interaction (retry/cancel).

Each phase delivers a working, testable increment.

## Design Decisions

| Decision                     | Choice                                               | Rationale                                                                                                                                                                            |
| ---------------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Architecture                 | DAG Executor (Approach C)                            | Aligns with existing TaskGraph; naturally parallel via `JoinSet` + `Semaphore`; simpler than Actor model; deterministic scheduling for debuggability                                 |
| Agent abstraction            | Stateful `AgentStrategy` trait                       | Agents are strategy objects driven by the DAG executor, not independent actors. They hold context (model client, tool registry) but are invoked by the executor, not self-scheduling |
| Execution mode               | Opt-in via `/plan` prefix                            | Backward compatible — existing single-step behavior preserved. DAG execution triggered explicitly                                                                                    |
| Failure policy               | `BlockDependents` by default + `skip_task` override  | Failures should not silently proceed; explicit user action required to override                                                                                                      |
| Retry model                  | LLM retries in infra layer, tool retries by strategy | Network/rate-limit retries are transparent to the agent; tool-call retries are agent decisions (LLM sees error and chooses)                                                          |
| Phase 1 scope                | Pure refactoring, zero new features                  | All existing tests must pass unchanged. New modules get their own unit tests                                                                                                         |
| Phase 2 planner intervention | `Decompose` + `AfterWorkersComplete` only            | `OnFailure` planner re-plan is deferred to future work; reduces complexity while still covering the common case                                                                      |
| GUI dependency               | No new chart library                                 | CSS + Vue recursive components for N-level tree. DAG edge rendering deferred to Phase 3+                                                                                             |

---

## Phase 1: Module Split (Pure Refactoring)

### Target module structure

```
crates/agent-runtime/src/
├── lib.rs                    # Public API re-exports
├── agents.rs                 # AgentRole, AgentStrategy trait (keep, extend)
├── task_graph.rs             # TaskGraph (keep, extend)
├── mcp_manager.rs            # McpServerManager (keep, unchanged)
├── facade_runtime.rs         # AppFacade impl → thin coordinator (~200 lines)
├── session.rs                # Session lifecycle + SessionProjection (new)
├── agent_loop.rs             # LLM loop + tool call chain (new)
├── permission.rs             # Permission request/resolve/pending queue (new)
├── memory_handler.rs         # <memory> marker extraction/storage/confirmation (new)
├── event_emitter.rs          # Event emission helpers + source_agent_id injection (new)
└── dag_executor.rs           # Phase 2: DAG-driven executor (new, stub only)
```

### Key interfaces

```rust
// session.rs
pub struct SessionManager<S: EventStore> { ... }
impl<S: EventStore> SessionManager<S> {
    pub async fn start_session(&self, req: StartSessionRequest) -> agent_core::Result<SessionId>;
    pub async fn switch_session(&self, id: &SessionId) -> agent_core::Result<SessionProjection>;
    pub async fn cancel_session(&self, id: &SessionId) -> agent_core::Result<()>;
    pub async fn list_sessions(&self) -> agent_core::Result<Vec<SessionMeta>>;
}

// agent_loop.rs
pub struct AgentLoop<S: EventStore, M: ModelClient> { ... }
impl<S: EventStore, M: ModelClient> AgentLoop<S, M> {
    pub async fn run_step(&self, ctx: &StepContext) -> agent_core::Result<StepOutcome>;
    pub async fn run_to_completion(&self, ctx: StepContext) -> agent_core::Result<()>;
}

pub struct StepContext {
    pub session_id: SessionId,
    pub workspace_id: WorkspaceId,
    pub user_message: String,
    pub source_agent_id: AgentId,
    pub cancellation: CancellationToken,
}

pub enum StepOutcome {
    Continue,
    Completed,
    Cancelled,
    PermissionRequired,
    MaxIterations,
}

// permission.rs
pub struct PermissionHandler { ... }
impl PermissionHandler {
    pub async fn check_permission(&self, invocation: &ToolInvocation) -> PermissionOutcome;
    pub async fn resolve_permission(&self, request_id: &str, decision: PermissionDecision) -> bool;
}

// memory_handler.rs
pub struct MemoryHandler { ... }
impl MemoryHandler {
    pub async fn process_response_markers(&self, response: &str, session_id: &SessionId) -> (String, Vec<MemoryProposal>);
    pub async fn confirm_memory(&self, proposal_id: &str, decision: PermissionDecision) -> agent_core::Result<()>;
}

// event_emitter.rs
pub struct EventEmitter { ... }
impl EventEmitter {
    pub fn emit(&self, event: DomainEvent) -> agent_core::Result<()>;
    pub fn with_agent(&self, agent_id: AgentId) -> EventEmitter<'_>;
}
```

### `facade_runtime.rs` after split

```rust
impl<S: EventStore, M: ModelClient> AppFacade for LocalRuntime<S, M> {
    async fn start_session(&self, req: StartSessionRequest) -> ... {
        self.session_manager.start_session(req).await
    }
    async fn send_message(&self, req: SendMessageRequest) -> ... {
        let ctx = self.build_step_context(req);
        self.agent_loop.run_to_completion(ctx).await
    }
    // ... other AppFacade methods delegate to modules
}
```

### Testing

- **Refactor baseline tests** in `tests/refactor_baseline.rs`: end-to-end behavior tests for every public `AppFacade` method, must pass before and after refactoring
- **New module unit tests**: each new module (`session`, `agent_loop`, `permission`, `memory_handler`, `event_emitter`) gets `#[cfg(test)] mod tests`
- **Zero behavior change**: all existing tests pass unchanged

---

## Phase 2: DAG Execution + Agent Strategy

### Data flow

```
User message → LocalRuntime.send_message()
                  │
                  ├── SingleStep mode → AgentLoop.run_to_completion() (current behavior)
                  │
                  └── DagExecution mode → DagExecutor.execute()
                        │
                        ▼
                   PlannerStrategy.decompose() → TaskGraph (DAG)
                        │
                        ▼
                   DagExecutor scheduling loop:
                     ┌──────────────────────────────────┐
                     │  ready_tasks() → spawn in JoinSet  │
                     │  each task bound to AgentStrategy  │
                     │  execute run_step()                │
                     │  completed → mark_completed()      │
                     │  failed → mark_failed() + cascade  │
                     │  new ready_tasks → continue        │
                     └──────────────────────────────────┘
                        │
                        ▼ (all tasks done)
                   ReviewerStrategy.review() → approve or flag
                        │
                        ▼
                   Aggregate result → return to user
```

### Execution mode selection

```rust
enum ExecutionMode {
    SingleStep,      // Default: current agent loop behavior
    DagExecution,    // Opt-in: Planner decompose + parallel Workers
}

impl LocalRuntime {
    fn execution_mode(&self, ctx: &StepContext) -> ExecutionMode {
        if ctx.user_message.starts_with("/plan ") && self.dag_executor.is_available() {
            ExecutionMode::DagExecution
        } else {
            ExecutionMode::SingleStep
        }
    }
}
```

### AgentStrategy trait

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

pub enum AgentDecision {
    RequestModel { tools: Vec<ToolDef> },
    Respond(String),
    Decompose { sub_tasks: Vec<SubTaskDef> },
    ReviewComplete { approved: bool, findings: Vec<ReviewerFinding> },
}

pub enum ToolResultAction {
    Continue,
    Retry { max_retries: usize },
    Abort(String),
}

pub struct SubTaskDef {
    pub title: String,
    pub role: AgentRole,
    pub dependencies: Vec<TaskId>,
    pub description: String,
}
```

### Three built-in strategies

**PlannerStrategy**: Receives user goal, decomposes into sub-task DAG. System prompt instructs JSON output of `Vec<SubTaskDef>`. Simple questions that don't need decomposition return `AgentDecision::Respond`.

**WorkerStrategy**: Executes a specific sub-task. Receives task description + dependency outputs as context. Can call tools via `AgentLoop.run_step()`.

**ReviewerStrategy**: Reviews Worker output. Receives original task + Worker's output. Returns structured `ReviewComplete` with approval and findings.

### DagExecutor

```rust
pub struct DagExecutor<S: EventStore, M: ModelClient> {
    store: Arc<S>,
    model: Arc<M>,
    event_emitter: EventEmitter,
    permission_handler: PermissionHandler,
    memory_handler: MemoryHandler,
    strategies: HashMap<AgentRole, Arc<dyn AgentStrategy>>,
    max_concurrency: usize,
}

pub struct ExecutionResult {
    pub graph: TaskGraph,
    pub total_tasks: usize,
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
}
```

Key behaviors:

- `execute()`: Planner decompose → scheduling loop → Reviewer review
- Scheduling loop: `ready_tasks()` → `Semaphore`-bounded `JoinSet` → collect results → repeat
- Failure cascade: `BlockDependents` by default, with `retry_task()` and `skip_task()` for recovery
- Concurrency: configurable `max_concurrency` (default: 3)

### TaskGraph extensions

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Ready,       // Dependencies complete, waiting to be scheduled
    Running,
    Completed,
    Failed,
    Blocked,     // A dependency failed
    Skipped,     // User explicitly skipped
    Cancelled,   // User cancelled
}

pub struct AgentTask {
    pub id: TaskId,
    pub title: String,
    pub role: AgentRole,
    pub state: TaskState,
    pub dependencies: Vec<TaskId>,
    pub error: Option<String>,
    pub retry_count: usize,
    pub max_retries: usize,
    pub assigned_agent_id: Option<AgentId>,
    pub failure_reason: Option<TaskFailureReason>,
}
```

New methods: `mark_blocked`, `mark_skipped`, `reset_to_pending`, `mark_ready`, `find_blocked_dependents`, `find_direct_dependents`, `get_task`, `is_finished`.

### Failure model

```rust
pub enum FailurePolicy {
    BlockDependents,   // Default: block all transitive dependents
    AllowOrphans,      // Dependents receive "parent failed" context and may proceed
    FailFast,          // Cancel the entire DAG
}

pub enum TaskFailureReason {
    ModelError { source: ModelClientError, retries: usize },
    ToolExhausted { tool_id: String, attempts: usize, last_error: String },
    PermissionDenied { tool_id: String },
    Cancelled,
    MaxIterations,
}

pub struct RetryConfig {
    pub max_model_retries: usize,   // Default: 3
    pub max_tool_retries: usize,    // Default: 2
    pub backoff: BackoffStrategy,   // Default: ExponentialJitter
}
```

Task state machine:

```
Pending → Ready (deps complete) → Running → Completed
                                  Running → Failed → Pending (retry)
                                  Running → Cancelled
Failed → Skipped (user action)
Pending → Blocked (dep failed)
Blocked → Pending (dep retried)
Blocked → Skipped (user action)
```

### New event types (Phase 2)

```rust
// Added to EventPayload enum
TaskDecomposed { parent_task_id: String, sub_task_ids: Vec<String> },
TaskBlocked { task_id: String, blocking_task_id: String, reason: String },
AgentSpawned { agent_id: String, role: String, task_id: String },
AgentIdle { agent_id: String },
TaskRetried { task_id: String, attempt: usize },
```

### New Tauri commands (Phase 2)

- `retry_task(session_id, task_id)` — Retry a failed task, unblocking dependents
- `cancel_task(session_id, task_id)` — Cancel a specific task
- `get_agent_status(session_id)` — List active agents and their states

---

## Phase 3: GUI Evolution

### Event pipeline

```
Rust emit(event.with_agent_id) → event_forwarder → Tauri "session-event"
  → useTauriEvents.ts (route by event type)
    → session.ts (message attribution)
    → taskGraph.ts (DAG updates)
    → agents.ts (new: agent status tracking)
```

### New Agent Store

```typescript
// stores/agents.ts
export interface AgentInfo {
  id: string;
  role: AgentRole;
  taskId: string | null;
  status: 'idle' | 'running' | 'completed' | 'failed';
  startedAt: number;
  completedAt: number | null;
}
export const agentState = reactive({
  agents: new Map<string, AgentInfo>(),
  runningAgents: computed(() => ...),
  agentsByRole: computed(() => ...),
});
export function applyAgentEvent(payload: EventPayload) { ... }
export function clearAgents() { ... }
```

### Session Store changes

- `token_stream` → `streamsByTask: Map<string, string>` (with fallback for Phase 1 compatibility)
- `ProjectedRole` extends to `'user' | 'assistant' | 'planner' | 'worker' | 'reviewer' | 'system'`
- Messages gain optional `sourceAgentId` and `taskId` fields

### TaskGraph Store changes

- Replace 2-level `buildTaskTree` with N-level recursive tree builder using dependency inference
- Add `assignedAgentId`, `startedAt`, `completedAt`, `retryCount` fields to `TaskNode`

### TaskSteps.vue changes

- Recursive `<TaskNode>` component for N-level tree
- Agent role badges: `P` (Planner), `W:1` (Worker 1), `R` (Reviewer)
- Retry button for failed tasks
- Duration display for completed/running tasks
- Expanded/collapsed state per node

### PermissionPrompt.vue changes

- Permission queue (multiple pending requests)
- Agent attribution (which agent is requesting)
- "Approve all from this agent" bulk action

### New E2E specs

- `task-graph-interaction.spec.ts` — N-level tree, retry, agent badges
- `multi-agent-flow.spec.ts` — planner decomposition, parallel workers, blocked tasks

---

## Testing Strategy

### Phase 1

- **Refactor baseline tests** (6-10 tests): anchor all `AppFacade` behaviors before refactoring
- **New module unit tests** (25-30 tests): `session`, `agent_loop`, `permission`, `memory_handler`, `event_emitter`
- **All existing tests pass unchanged**

### Phase 2

- **TaskGraph state machine** (12+ tests): Pending→Ready→Running→Completed/Failed/Blocked/Skipped, cascade, retry
- **AgentStrategy** (6+ tests): Planner decompose, Worker tool calls, Reviewer approve/reject (using `FakeModelClient`)
- **DagExecutor integration** (9+ tests): linear DAG, parallel DAG, failure cascade, retry, skip, cancellation, concurrency limit
- **ExecutionMode** (2+ tests): single-step unchanged, `/plan` triggers DAG

### Phase 3

- **Vitest**: `agents.test.ts`, extended `taskGraph.test.ts`, extended `session.test.ts`
- **Playwright E2E**: `task-graph-interaction.spec.ts`, `multi-agent-flow.spec.ts`

### FakeModelClient extension

Add `role_responses: HashMap<AgentRole, Vec<ModelResponse>>` for DAG testing:

- Planner → returns sub-task JSON
- Worker → returns tool calls then completion
- Reviewer → returns approval/rejection

---

## Out of Scope

- **Cross-session agent collaboration** — agents work within a single session
- **OnFailure Planner re-plan** — deferred to future work
- **Agent-scoped memory** — memory stays session/user/workspace scoped
- **DAG visualization with edges** — deferred to Phase 3+ (CSS tree is sufficient)
- **Dynamic permission mode switching** — `set_permission_mode` Tauri command exists but UI toggle deferred
- **Per-agent MCP access control** — all agents share MCP servers

---

## Risks and Mitigations

| Risk                                   | Impact | Mitigation                                                                             |
| -------------------------------------- | ------ | -------------------------------------------------------------------------------------- |
| Phase 1 refactoring breaks behavior    | High   | Refactor baseline tests + run full suite after each module move                        |
| DAG executor concurrency bugs          | Medium | Comprehensive integration tests with `FakeModelClient`; `Semaphore` bounds parallelism |
| Planner generates invalid DAG (cycles) | Medium | `TaskGraph::add_task` validates acyclicity; fallback to single-step on error           |
| GUI token_stream race conditions       | Medium | `streamsByTask` is per-task, eliminating overwrites; Vue reactivity handles dedup      |
| Phase 2 events break Phase 1 frontend  | Low    | New event types are additive; frontend `switch` statements have `default` branch       |

---

## Phase Summary

| Phase       | Deliverables                                                                              | Tests     | Duration Estimate |
| ----------- | ----------------------------------------------------------------------------------------- | --------- | ----------------- |
| **Phase 1** | Module split, `run_step()`, `StepOutcome`, refactor baseline                              | 30-40 new | 2-3 sessions      |
| **Phase 2** | `AgentStrategy`, `DagExecutor`, `TaskGraph` extensions, new events, retry/cancel commands | 30-40 new | 4-5 sessions      |
| **Phase 3** | Agent store, N-level tree, permission queue, agent attribution, E2E                       | 20-30 new | 2-3 sessions      |

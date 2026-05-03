# Task Graph Visualization — Design Spec

**Date:** 2026-05-03
**Status:** Approved
**Scope:** Runtime TaskGraph integration, AppFacade task graph API, GUI task steps view, TUI task view density

---

## Problem

Kairox has a `TaskGraph` data structure with `AgentRole` (Planner/Worker/Reviewer) and `TaskState` (Pending/Running/Blocked/Completed/Failed/Cancelled), and `AgentTask*` event variants in `EventPayload`, but none of these are connected:

1. **TaskGraph is unused in runtime** — `facade_runtime.rs` never creates task nodes or emits task events during the agent loop
2. **AgentTask\* events are never emitted** — `AgentTaskCreated`, `AgentTaskStarted`, `AgentTaskCompleted`, `AgentTaskFailed` exist in the enum but are never constructed
3. **No facade API for task graph** — `AppFacade` has no method to query the current task graph state
4. **GUI has no task view** — The right panel only shows a flat trace event list; users cannot see task structure, dependencies, or progress
5. **TUI has no task view** — The trace panel renders tool/memory events but not task hierarchy

Users have no way to understand what the agent is doing, which tasks are running, which failed, or how they relate to each other.

## Design Decisions

| Decision                | Choice                                                        | Rationale                                                                             |
| ----------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| Task creation model     | Root task per user message + sub-tasks per tool call          | Reflects real structure (multi-tool fan-out); no fake phases                          |
| TaskGraph storage       | Per-session `HashMap<SessionId, TaskGraph>` in `LocalRuntime` | Session isolation; simple lifecycle management                                        |
| API style               | Snapshot (`get_task_graph`) not streaming                     | Existing `AgentTask*` events are natural refresh triggers; no new event types needed  |
| GUI layout              | Tab within Trace panel (Trace / Tasks)                        | Reuses existing space; natural context switch; no layout change                       |
| TUI layout              | 4th density mode (TaskGraph) in trace panel                   | Follows existing density cycling pattern                                              |
| State machine extension | `Pending → Running → Completed/Failed` on `TaskGraph`         | Current `TaskGraph` only has `mark_completed`; needs `mark_running` and `mark_failed` |
| `AgentTask.error` field | New `Option<String>` field on `AgentTask`                     | Failed tasks need error info for display                                              |

## Architecture

### Agent Loop Integration

Current flow:

```
send_message()
  → emit UserMessageAdded
  → loop:
      → ContextAssembled / ModelRequestStarted
      → handle response: text or tool_call
      → Permission → Tool.invoke()
      → if no tool calls: break
```

New flow:

```
send_message()
  → emit UserMessageAdded
  → root_task = task_graph.add_task(summary, Planner, [])
  → emit AgentTaskCreated { root_task }
  → mark_running(root_task)
  → emit AgentTaskStarted { root_task }
  → loop:
      → ContextAssembled / ModelRequestStarted
      → if model_response.has_tool_calls:
          → for each tool_call:
              → sub_task = task_graph.add_task(tool_id, Worker, [root_task])
              → emit AgentTaskCreated { sub_task }
              → mark_running(sub_task)
              → emit AgentTaskStarted { sub_task }
              → Permission → Tool.invoke()
              → on success: mark_completed(sub_task); emit AgentTaskCompleted
              → on failure: mark_failed(sub_task, error); emit AgentTaskFailed
          → continue loop
      → if no tool calls (final response):
          → mark_completed(root_task)
          → emit AgentTaskCompleted { root_task }
```

### TaskGraph State Machine Extension

Current:

```
Pending ──mark_completed()──→ Completed
(mark_completed on unknown TaskId → error)
```

New:

```
Pending ──mark_running()──→ Running ──mark_completed()──→ Completed
                                └──mark_failed(err)──→ Failed

- mark_running on unknown TaskId → error
- mark_running on non-Pending → no-op (idempotent)
- mark_completed on Pending (skip Running) → allowed (simplification)
- mark_failed on Pending → transitions to Failed directly
```

### Data Model

```rust
// agent-core

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub id: TaskId,
    pub title: String,
    pub role: AgentRole,  // from agent-runtime, re-exported or mirrored
    pub state: TaskState,
    pub dependencies: Vec<TaskId>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGraphSnapshot {
    pub tasks: Vec<TaskSnapshot>,
}
```

### AppFacade Extension

```rust
// New method on AppFacade trait
async fn get_task_graph(&self, session_id: SessionId) -> crate::Result<TaskGraphSnapshot>;
```

### SessionProjection Extension

```rust
pub struct SessionProjection {
    pub messages: Vec<ProjectedMessage>,
    pub task_titles: Vec<String>,        // preserved for backward compat
    pub task_graph: TaskGraphSnapshot,    // NEW
    pub token_stream: String,
    pub cancelled: bool,
}
```

`apply()` handles `AgentTask*` events to build `task_graph` incrementally.

### GUI Refresh Strategy

No new event types. GUI refreshes on existing `AgentTask*` events:

```
AgentTaskCreated   → invoke("get_task_graph", { sessionId })
AgentTaskStarted   → invoke("get_task_graph", { sessionId })
AgentTaskCompleted → invoke("get_task_graph", { sessionId })
AgentTaskFailed    → invoke("get_task_graph", { sessionId })
```

### GUI Component Structure

```
TraceTimeline.vue
  ├── [Tab: Trace]  → existing TraceEntry list (unchanged)
  └── [Tab: Tasks]  → TaskSteps.vue (NEW)
        └── TaskTreeNode items with indent, status icons, role badges
```

### TUI Component Changes

`TraceDensity` gains a 4th variant `TaskGraph`. When selected, the trace panel renders `extract_task_traces()` output with indented tree structure instead of flat event list.

## File Changes Summary

| Layer    | File                                               | Change                                                                                  |
| -------- | -------------------------------------------------- | --------------------------------------------------------------------------------------- |
| core     | `crates/agent-core/src/events.rs`                  | No changes (AgentTask\* variants already exist)                                         |
| core     | `crates/agent-core/src/facade.rs`                  | +`TaskSnapshot`, `TaskGraphSnapshot`, +`get_task_graph` trait method, +NoopFacade impl  |
| core     | `crates/agent-core/src/projection.rs`              | +`task_graph` field on `SessionProjection`, +`AgentTask*` apply handlers                |
| core     | `crates/agent-core/src/lib.rs`                     | Re-export new types                                                                     |
| runtime  | `crates/agent-runtime/src/task_graph.rs`           | +`error` field on `AgentTask`, +`mark_running()`, +`mark_failed()`, +`snapshot()`       |
| runtime  | `crates/agent-runtime/src/facade_runtime.rs`       | +`task_graphs` field, inject task tracking in agent loop, +`get_task_graph` impl        |
| runtime  | `crates/agent-runtime/src/lib.rs`                  | Re-export as needed                                                                     |
| gui-rust | `apps/agent-gui/src-tauri/src/commands.rs`         | +`get_task_graph` Tauri command                                                         |
| gui-rust | `apps/agent-gui/src-tauri/src/specta.rs`           | Register new command + types                                                            |
| gui-rust | `apps/agent-gui/src-tauri/src/lib.rs`              | Register command in handler                                                             |
| gui-vue  | `apps/agent-gui/src/components/TaskSteps.vue`      | NEW: task steps tree view component                                                     |
| gui-vue  | `apps/agent-gui/src/components/TraceTimeline.vue`  | Add Tab switcher (Trace / Tasks)                                                        |
| gui-vue  | `apps/agent-gui/src/stores/taskGraph.ts`           | NEW: task graph state + tree builder                                                    |
| gui-vue  | `apps/agent-gui/src/composables/useTauriEvents.ts` | Refresh task graph on AgentTask\* events                                                |
| gui-vue  | `apps/agent-gui/src/types/index.ts`                | +AgentRole, TaskState, TaskSnapshot, TaskGraphSnapshot                                  |
| gui-ts   | `apps/agent-gui/src/generated/commands.ts`         | Regenerated via `just gen-types`                                                        |
| tui      | `crates/agent-tui/src/components/trace.rs`         | +`TaskGraph` density, +`extract_task_traces()`, +`render_task_graph()`, +`TaskTreeNode` |
| tui      | `crates/agent-tui/src/keybindings.rs`              | Update density cycling: Summary → Expanded → FullEventStream → TaskGraph → Summary      |

## Testing Strategy

| Layer            | Test                                                | What it verifies                                                    |
| ---------------- | --------------------------------------------------- | ------------------------------------------------------------------- |
| agent-core       | `SessionProjection::apply` for `AgentTask*` events  | task_graph field updates correctly through state transitions        |
| agent-core       | `TaskGraphSnapshot` serde roundtrip                 | Serialization consistency                                           |
| agent-runtime    | `TaskGraph::mark_running` / `mark_failed`           | State transitions and error paths                                   |
| agent-runtime    | `TaskGraph::snapshot`                               | Snapshot contains all tasks with correct state                      |
| agent-runtime    | `send_message` emits task events (single tool call) | Event sequence: Created→Started→Created→Started→Completed→Completed |
| agent-runtime    | `send_message` with parallel tool calls             | Fan-out: two Worker sub-tasks share root Planner dependency         |
| agent-runtime    | `get_task_graph` API                                | Returns correct snapshot after agent loop                           |
| agent-runtime    | Tool failure creates Failed subtask                 | AgentTaskFailed emitted, error field populated                      |
| agent-runtime    | Plain message (no tool calls)                       | Root task Completed, zero sub-tasks                                 |
| agent-tui        | `extract_task_traces`                               | Hierarchy extraction from event list                                |
| agent-gui (Rust) | `get_task_graph` command                            | Parameter passing and return format                                 |

## Non-Goals

- Full DAG canvas rendering with nodes and edges (future: multi-agent orchestration UX)
- Task cancellation from UI
- Task retry from UI
- Multi-agent parallel execution (Worker sub-tasks still execute sequentially within agent loop)
- Drag-and-drop task reordering
- Task filtering or search
- Persisting TaskGraph to SQLite (task graph is ephemeral, rebuilt from events)

# TUI Task And Memory Right Panel Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the TUI right panel closer to GUI parity by making it tab-like across trace, tasks, and memories, adding task retry/cancel actions, and adding memory query/delete.

**Architecture:** Keep the change inside the TUI right panel surface. `TracePanel` owns right-panel UI state and pure extraction/render helpers; the app layer routes key actions into `Command` variants; `main.rs` executes runtime and memory-store calls. Reuse existing `TaskGraphSnapshot`, `retry_task`, `cancel_task`, and `MemoryStore` paths instead of adding new facade APIs.

**Tech Stack:** Rust, ratatui, crossterm, Tokio, existing `agent-core` facade DTOs, existing `agent-memory::MemoryStore`.

---

### Task 1: Task Hierarchy Extraction

**Files:**

- Modify: `crates/agent-tui/src/components/trace.rs`

- [ ] **Step 1: Write the failing test**

Add a test near the existing trace tests:

```rust
#[test]
fn builds_task_tree_from_snapshot_dependencies() {
    use agent_core::{AgentRole, TaskId, TaskSnapshot, TaskState};
    use agent_core::facade::TaskGraphSnapshot;

    let root_id = TaskId::from_string("task_root".into());
    let child_id = TaskId::from_string("task_child".into());
    let grandchild_id = TaskId::from_string("task_grandchild".into());

    let snapshot = TaskGraphSnapshot {
        tasks: vec![
            task_snapshot(root_id.clone(), "Plan", AgentRole::Planner, TaskState::Completed, vec![]),
            task_snapshot(child_id.clone(), "Build", AgentRole::Worker, TaskState::Failed, vec![root_id.clone()]),
            task_snapshot(grandchild_id.clone(), "Review", AgentRole::Reviewer, TaskState::Blocked, vec![child_id.clone()]),
        ],
    };

    let tree = build_task_tree_from_snapshot(&snapshot);

    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].id, "task_root");
    assert_eq!(tree[0].children[0].id, "task_child");
    assert_eq!(tree[0].children[0].children[0].id, "task_grandchild");
    assert_eq!(tree[0].children[0].status, TraceStatus::Failed);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-tui trace::tests::builds_task_tree_from_snapshot_dependencies`

Expected: FAIL because `build_task_tree_from_snapshot` and `task_snapshot` do not exist.

- [ ] **Step 3: Write minimal implementation**

Add a `TaskTreeNode` shape that keeps retry metadata, map `TaskState` into `TraceStatus`, and build parent/child relationships from the last known dependency present in the snapshot. Keep the existing event-based extraction for trace history compatibility.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p agent-tui trace::tests::builds_task_tree_from_snapshot_dependencies`

Expected: PASS.

### Task 2: Tab-Like Right Panel State

**Files:**

- Modify: `crates/agent-tui/src/components/trace.rs`
- Modify: `crates/agent-tui/src/app/render.rs`
- Modify: `crates/agent-tui/src/app/input.rs`
- Modify: `crates/agent-tui/src/keybindings/action.rs`
- Modify: `crates/agent-tui/src/keybindings/resolver.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving `TracePanel` cycles tabs independently from the existing density cycle, and that `F5` still maps to density when Trace has focus.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-tui trace keybindings::tests::l4_f5_toggles_trace_in_trace_focus`

Expected: FAIL for the new tab-cycle behavior before implementation; existing F5 test remains PASS.

- [ ] **Step 3: Write minimal implementation**

Add `RightPanelTab::{Trace, Tasks, Memory}` and `TracePanel::cycle_tab()`. Bind a focused trace-panel key such as `[`/`]` or `h`/`l` to tab cycling without changing `F5` density behavior. Render titles as `Trace | Tasks | Memory`, highlighting the selected tab.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p agent-tui trace keybindings`

Expected: PASS.

### Task 3: Task Actions

**Files:**

- Modify: `crates/agent-tui/src/components/mod.rs`
- Modify: `crates/agent-tui/src/components/trace.rs`
- Modify: `crates/agent-tui/src/app/input.rs`
- Modify: `crates/agent-tui/src/main.rs`

- [ ] **Step 1: Write failing command tests**

Add unit tests that select a failed task in the Tasks tab and assert:

```rust
assert_eq!(
    commands,
    vec![Command::RetryTask {
        workspace_id: workspace_id.clone(),
        session_id: session_id.clone(),
        task_id: failed_task_id.clone(),
    }]
);
```

Add a second test for cancelling a running task with `Command::CancelTask`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p agent-tui trace::tests::tasks_tab_emits_retry_command trace::tests::tasks_tab_emits_cancel_command`

Expected: FAIL because commands and handlers do not exist.

- [ ] **Step 3: Write minimal implementation**

Add `Command::RetryTask` and `Command::CancelTask`, route keys only when the right panel is focused on Tasks, and dispatch them through `runtime.retry_task(workspace_id, session_id, task_id)` and `runtime.cancel_task(workspace_id, session_id, task_id)`.

- [ ] **Step 4: Run focused tests**

Run: `cargo test -p agent-tui trace task`

Expected: PASS.

### Task 4: Memory Browser Query/Delete

**Files:**

- Modify: `crates/agent-tui/src/components/mod.rs`
- Modify: `crates/agent-tui/src/components/trace.rs`
- Modify: `crates/agent-tui/src/app/input.rs`
- Modify: `crates/agent-tui/src/main.rs`

- [ ] **Step 1: Write failing memory tests**

Add tests for a memory row model and deletion command:

```rust
assert_eq!(
    commands,
    vec![Command::DeleteMemory {
        memory_id: "mem_user".into(),
    }]
);
```

Add a render or helper test that query results show scope, key, and content preview.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p agent-tui trace::tests::memory_tab_emits_delete_command trace::tests::memory_rows_render_scope_key_and_preview`

Expected: FAIL because memory rows and delete command do not exist.

- [ ] **Step 3: Write minimal implementation**

Add `MemoryRow`, `TracePanel::set_memories`, `Command::LoadMemories`, and `Command::DeleteMemory`. Load memory rows on Memory tab entry and delete the selected row through the existing `MemoryStore::delete`, then reload query results with a 100-row limit.

- [ ] **Step 4: Run focused tests**

Run: `cargo test -p agent-tui trace memory`

Expected: PASS.

### Task 5: Verification And PR

**Files:**

- Modify only the files above unless focused tests expose a required adjacent fix.

- [ ] **Step 1: Run focused verification**

Run: `cargo test -p agent-tui trace task memory`

Expected: PASS.

- [ ] **Step 2: Run required repository checks**

Run:

```bash
bun run format:check
bun run lint
cargo test --workspace --all-targets
```

Expected: PASS.

- [ ] **Step 3: Commit and open PR**

Commit message: `feat(tui): add task and memory right panel`

Create PR against `iter/tui-parity`, enable squash auto-merge, watch CI, confirm merge, then clean up only `feat/tui-task-memory-panel`.

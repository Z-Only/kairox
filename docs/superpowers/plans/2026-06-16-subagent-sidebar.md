# Subagent Sidebar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a right-sidebar Subagents tab that lets users inspect active planner/worker/reviewer agents and manage their bound tasks.

**Architecture:** Reuse the existing event-fed `useAgentsStore()` and `useTaskGraphStore()` instead of adding backend state. The new panel derives agent rows from the agent map and task graph, then calls the existing `retryTask()` / `cancelTask()` task actions for management.

**Tech Stack:** Vue 3 SFC, Pinia setup stores, vue-i18n, Vitest, existing Kx UI primitives.

---

## File Structure

- Create `apps/agent-gui/src/components/SubagentPanel.vue`: right-sidebar panel that renders agent summary, filters, task metadata, and retry/cancel actions.
- Create `apps/agent-gui/src/components/SubagentPanel.test.ts`: RED/GREEN component coverage for viewing, filtering, localization, and task management actions.
- Modify `apps/agent-gui/src/components/TraceTimeline.vue`: add the `Subagents` tab and render `SubagentPanel`.
- Modify `apps/agent-gui/src/components/TraceTimeline.test.ts`: assert tab activation and rendering.
- Modify `apps/agent-gui/src/stores/workspaceUi.ts` and `apps/agent-gui/src/stores/workspaceUi.test.ts`: add `subagents` as a persisted right-panel tab value.
- Modify `apps/agent-gui/src/locales/en.json` and `apps/agent-gui/src/locales/zh-CN.json`: add tab and panel strings.

Forbidden files for this task:

- `apps/agent-gui/src/generated/**`: no generated bindings are needed.
- `crates/**`: existing backend task retry/cancel and agent/task events already cover this UI.
- Existing unrelated dirty files unless a test failure directly proves the subagent panel requires them.

## Acceptance Signals

- Right sidebar exposes a `Subagents` tab with `data-test="trace-tab-subagents"`.
- The tab shows planner/worker/reviewer rows from `useAgentsStore()`.
- Each row shows role, agent label, status, current task title/state, and error text when present.
- Users can filter all/running/attention/done agents.
- A bound Failed or Blocked task below max retries shows a retry action that calls `taskGraph.retryTask(task.id)`.
- A bound non-terminal task shows a cancel action that calls `taskGraph.cancelTask(task.id)`.
- Empty state is localized in English and Chinese.

## Task 1: RED Tests for the Subagent Panel

**Files:**

- Create: `apps/agent-gui/src/components/SubagentPanel.test.ts`
- Test: `apps/agent-gui/src/components/SubagentPanel.test.ts`

- [ ] **Step 1: Write the failing component tests**

```ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import SubagentPanel from "./SubagentPanel.vue";
import { useAgentsStore } from "@/stores/agents";
import { useTaskGraphStore } from "@/stores/taskGraph";
import type { TaskSnapshot } from "@/types";
import en from "@/locales/en.json";
import zhCN from "@/locales/zh-CN.json";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

function makeTask(id: string, overrides?: Partial<TaskSnapshot>): TaskSnapshot {
  return {
    id,
    title: `Task ${id}`,
    role: "Worker",
    state: "Pending",
    dependencies: [],
    error: null,
    retry_count: 0,
    max_retries: 3,
    assigned_agent_id: null,
    failure_reason: null,
    ...overrides
  };
}

function mountPanel(locale: "en" | "zh-CN" = "en") {
  return mountWithPlugins(SubagentPanel, { reusePinia: true, locale }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("SubagentPanel", () => {
  it("shows a localized empty state when there are no subagents", () => {
    const wrapper = mountPanel();
    expect(wrapper.get('[data-test="subagent-panel"]').text()).toContain(en.subagents.empty);
  });

  it("renders localized empty state in Chinese", () => {
    const wrapper = mountPanel("zh-CN");
    expect(wrapper.get('[data-test="subagent-panel"]').text()).toContain(zhCN.subagents.empty);
  });

  it("renders subagents with role, label, status, and bound task", () => {
    const agents = useAgentsStore();
    const taskGraph = useTaskGraphStore();
    agents.applyAgentEvent({
      type: "AgentSpawned",
      agent_id: "agent_worker_1",
      role: "Worker",
      task_id: "task_1"
    });
    taskGraph.setTaskGraph(
      [
        makeTask("task_1", {
          title: "Implement sidebar",
          role: "Worker",
          state: "Running",
          assigned_agent_id: "agent_worker_1"
        })
      ],
      "ses_1"
    );

    const wrapper = mountPanel();

    expect(wrapper.get('[data-test="subagent-summary"]').text()).toContain("1");
    expect(wrapper.get('[data-test="subagent-card-agent_worker_1"]').text()).toContain("Worker");
    expect(wrapper.get('[data-test="subagent-card-agent_worker_1"]').text()).toContain("W");
    expect(wrapper.get('[data-test="subagent-card-agent_worker_1"]').text()).toContain("running");
    expect(wrapper.get('[data-test="subagent-card-agent_worker_1"]').text()).toContain(
      "Implement sidebar"
    );
    expect(wrapper.get('[data-test="subagent-card-agent_worker_1"]').text()).toContain("Running");
  });

  it("filters attention agents with failed or blocked bound tasks", async () => {
    const agents = useAgentsStore();
    const taskGraph = useTaskGraphStore();
    agents.applyAgentEvent({
      type: "AgentSpawned",
      agent_id: "agent_ok",
      role: "Worker",
      task_id: "ok"
    });
    agents.applyAgentEvent({
      type: "AgentSpawned",
      agent_id: "agent_failed",
      role: "Reviewer",
      task_id: "failed"
    });
    taskGraph.setTaskGraph(
      [
        makeTask("ok", { title: "Healthy task", state: "Running", assigned_agent_id: "agent_ok" }),
        makeTask("failed", {
          title: "Review failure",
          role: "Reviewer",
          state: "Failed",
          error: "Model failed",
          assigned_agent_id: "agent_failed"
        })
      ],
      "ses_1"
    );

    const wrapper = mountPanel();
    await wrapper.get('[data-test="subagent-filter-attention"]').trigger("click");

    expect(wrapper.text()).toContain("Review failure");
    expect(wrapper.text()).not.toContain("Healthy task");
    expect(wrapper.text()).toContain("Model failed");
  });

  it("calls retry and cancel task actions for the bound task", async () => {
    const agents = useAgentsStore();
    const taskGraph = useTaskGraphStore();
    const retryTask = vi.spyOn(taskGraph, "retryTask").mockResolvedValue(undefined);
    const cancelTask = vi.spyOn(taskGraph, "cancelTask").mockResolvedValue(undefined);
    agents.applyAgentEvent({
      type: "AgentSpawned",
      agent_id: "agent_failed",
      role: "Worker",
      task_id: "failed"
    });
    taskGraph.setTaskGraph(
      [
        makeTask("failed", {
          title: "Broken worker",
          state: "Failed",
          retry_count: 1,
          max_retries: 3,
          assigned_agent_id: "agent_failed"
        })
      ],
      "ses_1"
    );

    const wrapper = mountPanel();
    await wrapper.get('[data-test="subagent-retry-agent_failed"]').trigger("click");
    await wrapper.get('[data-test="subagent-cancel-agent_failed"]').trigger("click");

    expect(retryTask).toHaveBeenCalledWith("failed");
    expect(cancelTask).toHaveBeenCalledWith("failed");
  });
});
```

- [ ] **Step 2: Run the RED test**

Run:

```bash
cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test -- src/components/SubagentPanel.test.ts
```

Expected: FAIL because `SubagentPanel.vue` and `subagents` locale keys do not exist.

## Task 2: Implement the Subagent Panel

**Files:**

- Create: `apps/agent-gui/src/components/SubagentPanel.vue`
- Test: `apps/agent-gui/src/components/SubagentPanel.test.ts`

- [ ] **Step 1: Add the component implementation**

Implementation requirements:

- Use `useAgentsStore()` and `useTaskGraphStore()`.
- Sort agents by `startedAt`, then `id`.
- Derive `taskById` from `taskGraph.tasks`.
- `attention` filter matches agents with a bound task whose state is `Failed` or `Blocked`, or agents whose status is `failed`.
- `done` filter matches `completed`, `idle`, or a terminal bound task.
- `canRetry(task)` returns true only for Failed/Blocked tasks below retry budget.
- `canCancel(task)` returns true for Pending/Ready/Running/Blocked/Failed tasks.

- [ ] **Step 2: Run the panel test**

Run:

```bash
cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test -- src/components/SubagentPanel.test.ts
```

Expected: PASS with non-zero tests.

## Task 3: Add the Right Sidebar Tab

**Files:**

- Modify: `apps/agent-gui/src/stores/workspaceUi.ts`
- Modify: `apps/agent-gui/src/stores/workspaceUi.test.ts`
- Modify: `apps/agent-gui/src/components/TraceTimeline.vue`
- Modify: `apps/agent-gui/src/components/TraceTimeline.test.ts`
- Modify: `apps/agent-gui/src/locales/en.json`
- Modify: `apps/agent-gui/src/locales/zh-CN.json`

- [ ] **Step 1: Write the RED tab/store assertions**

Add to `workspaceUi.test.ts`:

```ts
it("accepts subagents as a right panel tab", () => {
  const store = useWorkspaceUiStore();
  store.setRightPanelTab("subagents");
  expect(store.rightPanelTab).toBe("subagents");
});
```

Add to `TraceTimeline.test.ts`:

```ts
it("switches to Subagents tab and renders the subagent panel", async () => {
  const wrapper = mountTimeline();
  await wrapper.get('[data-test="trace-tab-subagents"]').trigger("click");
  expect(useWorkspaceUiStore().rightPanelTab).toBe("subagents");
  expect(wrapper.get('[data-test="subagent-panel"]').exists()).toBe(true);
});
```

- [ ] **Step 2: Run the RED tab/store tests**

Run:

```bash
cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test -- src/stores/workspaceUi.test.ts src/components/TraceTimeline.test.ts
```

Expected: FAIL because `RightPanelTab` does not include `subagents`, and the tab selector is missing.

- [ ] **Step 3: Implement the tab**

Add `"subagents"` to `RightPanelTab`, import `SubagentPanel` in `TraceTimeline.vue`, add a `KxButton` with `data-test="trace-tab-subagents"` and `t("trace.tabSubagents")`, render `<SubagentPanel v-if="rightPanelTab === 'subagents'" />`, and add locale keys:

```json
"trace": {
  "tabSubagents": "Subagents"
},
"subagents": {
  "empty": "No subagents yet",
  "filterAll": "All",
  "filterRunning": "Running",
  "filterAttention": "Needs attention",
  "filterDone": "Done"
}
```

Chinese keys:

```json
"trace": {
  "tabSubagents": "子代理"
},
"subagents": {
  "empty": "暂无子代理",
  "filterAll": "全部",
  "filterRunning": "运行中",
  "filterAttention": "需处理",
  "filterDone": "已完成"
}
```

- [ ] **Step 4: Run focused GUI tests**

Run:

```bash
cd apps/agent-gui && NODE_OPTIONS="--localstorage-file=/tmp/kairox-vitest-localstorage.json" bun run test -- src/components/SubagentPanel.test.ts src/components/TraceTimeline.test.ts src/stores/workspaceUi.test.ts
```

Expected: PASS with non-zero tests.

## Task 4: Quality Gates and Dev App Verification

**Files:**

- Verify all modified files.

- [ ] **Step 1: Run format check**

Run:

```bash
bun run format:check
```

Expected: PASS. If it fails on changed TS/Vue/JSON formatting, run `bun run format:web` and re-run the focused tests.

- [ ] **Step 2: Run GUI lint**

Run:

```bash
bun run lint:web
```

Expected: PASS.

- [ ] **Step 3: Run Dev App verification**

Run:

```bash
bun --filter agent-gui tauri dev --features pilot &
until tauri-pilot ping 2>/dev/null; do sleep 2; done
tauri-pilot snapshot -i
```

Manual assertions:

- Workbench right sidebar is visible.
- Click `Subagents`.
- `data-test="subagent-panel"` is visible.
- Empty state appears when no subagents exist, or subagent cards appear when a session has DAG agent events.
- `tauri-pilot logs --level error` shows no JS errors.

- [ ] **Step 4: Clean up Dev App**

Run:

```bash
lsof -nP -iTCP:1420 -sTCP:LISTEN | awk 'NR>1{print $2}' | xargs kill 2>/dev/null || true
```

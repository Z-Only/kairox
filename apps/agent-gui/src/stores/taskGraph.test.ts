import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useTaskGraphStore } from "@/stores/taskGraph";
import { useAgentsStore } from "@/stores/agents";
import type { TaskSnapshot } from "@/types";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockedInvoke = vi.mocked(invoke);

const mockToast = {
  success: vi.fn(),
  error: vi.fn(),
  warning: vi.fn(),
  info: vi.fn()
};

vi.mock("@/composables/useToast", () => ({
  useToast: () => mockToast
}));

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

function makeTask(overrides: Partial<TaskSnapshot> & Pick<TaskSnapshot, "id">): TaskSnapshot {
  return {
    title: `Task ${overrides.id}`,
    role: "Planner" as const,
    state: "Pending" as const,
    dependencies: [],
    error: null,
    retry_count: 0,
    max_retries: 3,
    assigned_agent_id: null,
    failure_reason: null,
    ...overrides
  };
}

describe("buildTaskTree", () => {
  it("returns empty array for empty list", () => {
    const taskGraph = useTaskGraphStore();
    expect(taskGraph.buildTaskTree([])).toEqual([]);
  });

  it("returns single root for a single node", () => {
    const taskGraph = useTaskGraphStore();
    const a = makeTask({ id: "A" });
    const tree = taskGraph.buildTaskTree([a]);
    expect(tree).toHaveLength(1);
    expect(tree[0].task.id).toBe("A");
    expect(tree[0].children).toEqual([]);
    expect(tree[0].agentLabel).toBeNull();
  });

  it("builds a linear chain A→B→C", () => {
    const taskGraph = useTaskGraphStore();
    const a = makeTask({ id: "A" });
    const b = makeTask({ id: "B", dependencies: ["A"] });
    const c = makeTask({ id: "C", dependencies: ["B"] });
    const tree = taskGraph.buildTaskTree([a, b, c]);

    expect(tree).toHaveLength(1);
    const root = tree[0];
    expect(root.task.id).toBe("A");
    expect(root.children).toHaveLength(1);

    const bNode = root.children[0];
    expect(bNode.task.id).toBe("B");
    expect(bNode.children).toHaveLength(1);

    const cNode = bNode.children[0];
    expect(cNode.task.id).toBe("C");
    expect(cNode.children).toHaveLength(0);
  });

  it("builds parallel children — root A has B and C", () => {
    const taskGraph = useTaskGraphStore();
    const a = makeTask({ id: "A" });
    const b = makeTask({ id: "B", dependencies: ["A"] });
    const c = makeTask({ id: "C", dependencies: ["A"] });
    const tree = taskGraph.buildTaskTree([a, b, c]);

    expect(tree).toHaveLength(1);
    const root = tree[0];
    expect(root.task.id).toBe("A");
    expect(root.children).toHaveLength(2);
    expect(root.children.map((ch) => ch.task.id).sort()).toEqual(["B", "C"]);
  });

  it("handles multiple independent roots", () => {
    const taskGraph = useTaskGraphStore();
    const a = makeTask({ id: "A" });
    const d = makeTask({ id: "D" });
    const tree = taskGraph.buildTaskTree([a, d]);

    expect(tree).toHaveLength(2);
    expect(tree.map((n) => n.task.id).sort()).toEqual(["A", "D"]);
  });

  it("handles N-level tree (3 levels deep)", () => {
    const taskGraph = useTaskGraphStore();
    const a = makeTask({ id: "A" });
    const b = makeTask({ id: "B", dependencies: ["A"] });
    const c = makeTask({ id: "C", dependencies: ["B"] });
    const d = makeTask({ id: "D", dependencies: ["C"] });
    const tree = taskGraph.buildTaskTree([a, b, c, d]);

    expect(tree).toHaveLength(1);
    expect(tree[0].task.id).toBe("A");
    expect(tree[0].children[0].task.id).toBe("B");
    expect(tree[0].children[0].children[0].task.id).toBe("C");
    expect(tree[0].children[0].children[0].children[0].task.id).toBe("D");
  });

  it("handles diamond DAG (A→B, A→C, B→D, C→D)", () => {
    const taskGraph = useTaskGraphStore();
    const a = makeTask({ id: "A" });
    const b = makeTask({ id: "B", dependencies: ["A"] });
    const c = makeTask({ id: "C", dependencies: ["A"] });
    const d = makeTask({ id: "D", dependencies: ["B", "C"] });
    const tree = taskGraph.buildTaskTree([a, b, c, d]);

    expect(tree).toHaveLength(1);
    expect(tree[0].task.id).toBe("A");
    expect(tree[0].children).toHaveLength(2);
    const cNode = tree[0].children.find((n) => n.task.id === "C");
    expect(cNode).toBeDefined();
    expect(cNode!.children).toHaveLength(1);
    expect(cNode!.children[0].task.id).toBe("D");
  });

  it("treats dangling dependency as root (no crash)", () => {
    const taskGraph = useTaskGraphStore();
    const b = makeTask({ id: "B", dependencies: ["missing_parent"] });
    const tree = taskGraph.buildTaskTree([b]);

    expect(tree).toHaveLength(1);
    expect(tree[0].task.id).toBe("B");
    expect(tree[0].children).toEqual([]);
  });

  it("populates agentLabel from assigned_agent_id when agent is in store", () => {
    const taskGraph = useTaskGraphStore();
    const agents = useAgentsStore();
    agents.agents.set("agent_w1", {
      id: "agent_w1",
      role: "Worker",
      taskId: "T1",
      status: "running",
      startedAt: Date.now(),
      completedAt: null
    });

    const t1 = makeTask({ id: "T1", assigned_agent_id: "agent_w1" });
    const tree = taskGraph.buildTaskTree([t1]);
    expect(tree[0].agentLabel).toBe("W");
  });
});

describe("taskGraph store state management", () => {
  it("setTaskGraph sets tasks and sessionId", () => {
    const taskGraph = useTaskGraphStore();
    const tasks = [makeTask({ id: "T1" }), makeTask({ id: "T2" })];
    taskGraph.setTaskGraph(tasks, "session-1");

    expect(taskGraph.tasks).toEqual(tasks);
    expect(taskGraph.currentSessionId).toBe("session-1");
  });

  it("clearTaskGraph clears tasks and sessionId", () => {
    const taskGraph = useTaskGraphStore();
    const tasks = [makeTask({ id: "T1" })];
    taskGraph.setTaskGraph(tasks, "session-1");
    taskGraph.clearTaskGraph();

    expect(taskGraph.tasks).toEqual([]);
    expect(taskGraph.currentSessionId).toBeNull();
  });
});

describe("applyTaskEvent", () => {
  it("TaskRetried resets task to Running and updates retry_count", () => {
    const taskGraph = useTaskGraphStore();
    const task = makeTask({ id: "T1", state: "Failed", error: "boom" });
    taskGraph.setTaskGraph([task], "session-1");

    taskGraph.applyTaskEvent({
      type: "TaskRetried",
      task_id: "T1",
      attempt: 2
    } as any);

    const updated = taskGraph.tasks.find((t) => t.id === "T1");
    expect(updated?.state).toBe("Running");
    expect(updated?.retry_count).toBe(2);
    expect(updated?.error).toBeNull();
  });

  it("TaskCancelled sets task state to Cancelled", () => {
    const taskGraph = useTaskGraphStore();
    const task = makeTask({ id: "T1", state: "Failed", error: "boom" });
    taskGraph.setTaskGraph([task], "session-1");

    taskGraph.applyTaskEvent({
      type: "TaskCancelled",
      task_id: "T1"
    } as any);

    const updated = taskGraph.tasks.find((t) => t.id === "T1");
    expect(updated?.state).toBe("Cancelled");
    expect(updated?.error).toBeNull();
  });
});

describe("retryTask", () => {
  it("invokes retry_task and shows success toast", async () => {
    mockedInvoke.mockResolvedValue(undefined);
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph([makeTask({ id: "T1" })], "session-1");

    await taskGraph.retryTask("T1");

    expect(mockedInvoke).toHaveBeenCalledWith("retry_task", {
      sessionId: "session-1",
      taskId: "T1"
    });
    expect(mockToast.success).toHaveBeenCalledWith("Task retry started");
  });

  it("shows warning when no active session", async () => {
    const taskGraph = useTaskGraphStore();

    await taskGraph.retryTask("T1");

    expect(mockToast.warning).toHaveBeenCalledWith("No active session");
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it("shows error toast on invoke failure", async () => {
    mockedInvoke.mockRejectedValue(new Error("Backend error"));
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph([makeTask({ id: "T1" })], "session-1");

    await taskGraph.retryTask("T1");

    expect(mockToast.error).toHaveBeenCalledWith(expect.stringContaining("Failed to retry task"));
  });
});

describe("cancelTask", () => {
  it("invokes cancel_task and shows success toast", async () => {
    mockedInvoke.mockResolvedValue(undefined);
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph([makeTask({ id: "T1" })], "session-1");

    await taskGraph.cancelTask("T1");

    expect(mockedInvoke).toHaveBeenCalledWith("cancel_task", {
      sessionId: "session-1",
      taskId: "T1"
    });
    expect(mockToast.success).toHaveBeenCalledWith("Task cancelled");
  });

  it("shows warning when no active session", async () => {
    const taskGraph = useTaskGraphStore();

    await taskGraph.cancelTask("T1");

    expect(mockToast.warning).toHaveBeenCalledWith("No active session");
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it("shows error toast on invoke failure", async () => {
    mockedInvoke.mockRejectedValue(new Error("Backend error"));
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph([makeTask({ id: "T1" })], "session-1");

    await taskGraph.cancelTask("T1");

    expect(mockToast.error).toHaveBeenCalledWith(expect.stringContaining("Failed to cancel task"));
  });
});

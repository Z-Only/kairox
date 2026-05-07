import { describe, it, expect, beforeEach } from "vitest";
import { taskGraphState, setTaskGraph, clearTaskGraph, buildTaskTree } from "./taskGraph";
import { agentState, clearAgents } from "./agents";
import type { TaskSnapshot } from "../types";

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
  beforeEach(() => {
    clearAgents();
  });

  it("returns empty array for empty list", () => {
    expect(buildTaskTree([])).toEqual([]);
  });

  it("returns single root for a single node", () => {
    const a = makeTask({ id: "A" });
    const tree = buildTaskTree([a]);
    expect(tree).toHaveLength(1);
    expect(tree[0].task.id).toBe("A");
    expect(tree[0].children).toEqual([]);
    expect(tree[0].agentLabel).toBeNull();
  });

  it("builds a linear chain A→B→C", () => {
    const a = makeTask({ id: "A" });
    const b = makeTask({ id: "B", dependencies: ["A"] });
    const c = makeTask({ id: "C", dependencies: ["B"] });
    const tree = buildTaskTree([a, b, c]);

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
    const a = makeTask({ id: "A" });
    const b = makeTask({ id: "B", dependencies: ["A"] });
    const c = makeTask({ id: "C", dependencies: ["A"] });
    const tree = buildTaskTree([a, b, c]);

    expect(tree).toHaveLength(1);
    const root = tree[0];
    expect(root.task.id).toBe("A");
    expect(root.children).toHaveLength(2);
    expect(root.children.map((ch) => ch.task.id).sort()).toEqual(["B", "C"]);
  });

  it("handles multiple independent roots", () => {
    const a = makeTask({ id: "A" });
    const d = makeTask({ id: "D" });
    const tree = buildTaskTree([a, d]);

    expect(tree).toHaveLength(2);
    expect(tree.map((n) => n.task.id).sort()).toEqual(["A", "D"]);
  });

  it("handles N-level tree (3 levels deep)", () => {
    const a = makeTask({ id: "A" });
    const b = makeTask({ id: "B", dependencies: ["A"] });
    const c = makeTask({ id: "C", dependencies: ["B"] });
    const d = makeTask({ id: "D", dependencies: ["C"] });
    const tree = buildTaskTree([a, b, c, d]);

    expect(tree).toHaveLength(1);
    expect(tree[0].task.id).toBe("A");
    expect(tree[0].children[0].task.id).toBe("B");
    expect(tree[0].children[0].children[0].task.id).toBe("C");
    expect(tree[0].children[0].children[0].children[0].task.id).toBe("D");
  });

  it("handles diamond DAG (A→B, A→C, B→D, C→D)", () => {
    const a = makeTask({ id: "A" });
    const b = makeTask({ id: "B", dependencies: ["A"] });
    const c = makeTask({ id: "C", dependencies: ["A"] });
    const d = makeTask({ id: "D", dependencies: ["B", "C"] });
    const tree = buildTaskTree([a, b, c, d]);

    // D depends on B and C; it becomes a child of C (last dependency by list order)
    expect(tree).toHaveLength(1);
    expect(tree[0].task.id).toBe("A");
    expect(tree[0].children).toHaveLength(2); // B and C
    const cNode = tree[0].children.find((n) => n.task.id === "C");
    expect(cNode).toBeDefined();
    expect(cNode!.children).toHaveLength(1);
    expect(cNode!.children[0].task.id).toBe("D");
  });

  it("treats dangling dependency as root (no crash)", () => {
    const b = makeTask({ id: "B", dependencies: ["missing_parent"] });
    const tree = buildTaskTree([b]);

    expect(tree).toHaveLength(1);
    expect(tree[0].task.id).toBe("B");
    expect(tree[0].children).toEqual([]);
  });

  it("populates agentLabel from assigned_agent_id when agent is in store", () => {
    agentState.agents.set("agent_w1", {
      id: "agent_w1",
      role: "Worker",
      taskId: "T1",
      status: "running",
      startedAt: Date.now(),
      completedAt: null
    });

    const t1 = makeTask({ id: "T1", assigned_agent_id: "agent_w1" });
    const tree = buildTaskTree([t1]);
    expect(tree[0].agentLabel).toBe("W");
  });
});

describe("taskGraph store state management", () => {
  beforeEach(() => {
    clearTaskGraph();
  });

  it("setTaskGraph sets tasks and sessionId", () => {
    const tasks = [makeTask({ id: "T1" }), makeTask({ id: "T2" })];
    setTaskGraph(tasks, "session-1");

    expect(taskGraphState.tasks).toEqual(tasks);
    expect(taskGraphState.currentSessionId).toBe("session-1");
  });

  it("clearTaskGraph clears tasks and sessionId", () => {
    const tasks = [makeTask({ id: "T1" })];
    setTaskGraph(tasks, "session-1");
    clearTaskGraph();

    expect(taskGraphState.tasks).toEqual([]);
    expect(taskGraphState.currentSessionId).toBeNull();
  });
});

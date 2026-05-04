import { describe, it, expect, beforeEach } from "vitest";
import {
  taskGraphState,
  setTaskGraph,
  clearTaskGraph,
  buildTaskTree
} from "./taskGraph";
import type { TaskSnapshot } from "../types";

function makeTask(
  overrides: Partial<TaskSnapshot> & Pick<TaskSnapshot, "id">
): TaskSnapshot {
  return {
    title: `Task ${overrides.id}`,
    role: "Planner" as const,
    state: "Pending" as const,
    dependencies: [],
    error: null,
    ...overrides
  };
}

describe("buildTaskTree", () => {
  it("returns empty array for empty list", () => {
    expect(buildTaskTree([])).toEqual([]);
  });

  it("returns single root for a single node", () => {
    const a = makeTask({ id: "A" });
    const tree = buildTaskTree([a]);
    expect(tree).toHaveLength(1);
    expect(tree[0].task.id).toBe("A");
    expect(tree[0].children).toEqual([]);
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

  it("treats dangling dependency as root (no crash)", () => {
    const b = makeTask({ id: "B", dependencies: ["missing_parent"] });
    const tree = buildTaskTree([b]);

    expect(tree).toHaveLength(1);
    expect(tree[0].task.id).toBe("B");
    expect(tree[0].children).toEqual([]);
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

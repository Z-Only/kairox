import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import TaskNode from "./TaskNode.vue";
import type { TaskTreeNode } from "@/stores/taskGraph";
import { useTaskGraphStore } from "@/stores/taskGraph";
import type { TaskSnapshot } from "../types";

// Use vi.hoisted so these are available inside the hoisted vi.mock factories
const { mockRetryTask, mockCancelTask } = vi.hoisted(() => ({
  mockRetryTask: vi.fn(),
  mockCancelTask: vi.fn()
}));

// Mock Tauri APIs
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

function makeNode(
  overrides: Partial<TaskSnapshot> & { id: string },
  children: TaskTreeNode[] = []
): TaskTreeNode {
  return {
    task: {
      title: `Task ${overrides.id}`,
      role: "Worker",
      state: "Pending",
      dependencies: [],
      error: null,
      retry_count: 0,
      max_retries: 3,
      assigned_agent_id: null,
      failure_reason: null,
      ...overrides
    },
    children,
    agentLabel: null,
    durationMs: null
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  // Replace store actions with hoisted spies so we can assert invocations.
  const taskGraph = useTaskGraphStore();
  taskGraph.retryTask = mockRetryTask;
  taskGraph.cancelTask = mockCancelTask;
});

describe("TaskNode", () => {
  describe("status icons", () => {
    it("renders correct status icon for Pending state", () => {
      const node = makeNode({ id: "t1", state: "Pending" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-status").text()).toBe("⏳");
    });

    it("renders correct status icon for Ready state", () => {
      const node = makeNode({ id: "t1", state: "Ready" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-status").text()).toBe("⏳");
    });

    it("renders correct status icon for Running state", () => {
      const node = makeNode({ id: "t1", state: "Running" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-status").text()).toBe("🔄");
    });

    it("renders correct status icon for Blocked state", () => {
      const node = makeNode({ id: "t1", state: "Blocked" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-status").text()).toBe("⏸️");
    });

    it("renders correct status icon for Completed state", () => {
      const node = makeNode({ id: "t1", state: "Completed" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-status").text()).toBe("✅");
    });

    it("renders correct status icon for Failed state", () => {
      const node = makeNode({ id: "t1", state: "Failed" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-status").text()).toBe("❌");
    });

    it("renders correct status icon for Skipped state", () => {
      const node = makeNode({ id: "t1", state: "Skipped" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-status").text()).toBe("⏭️");
    });

    it("renders correct status icon for Cancelled state", () => {
      const node = makeNode({ id: "t1", state: "Cancelled" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-status").text()).toBe("🚫");
    });
  });

  describe("role badges", () => {
    it("renders role badge with P for Planner", () => {
      const node = makeNode({ id: "t1", role: "Planner" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      const badge = wrapper.find(".task-role");
      expect(badge.text()).toBe("P");
      // jsdom converts hex to rgb in inline styles
      expect(badge.element.getAttribute("style")).toContain("rgb(0, 119, 204)");
    });

    it("renders role badge with W for Worker", () => {
      const node = makeNode({ id: "t1", role: "Worker" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      const badge = wrapper.find(".task-role");
      expect(badge.text()).toBe("W");
      expect(badge.element.getAttribute("style")).toContain("rgb(34, 160, 107)");
    });

    it("renders role badge with R for Reviewer", () => {
      const node = makeNode({ id: "t1", role: "Reviewer" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      const badge = wrapper.find(".task-role");
      expect(badge.text()).toBe("R");
      expect(badge.element.getAttribute("style")).toContain("rgb(124, 58, 237)");
    });
  });

  describe("retry indicator", () => {
    it("shows retry indicator when retry_count > 0", () => {
      const node = makeNode({ id: "t1", retry_count: 1, max_retries: 3 });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-retry").text()).toBe("↻1/3");
    });

    it("does not show retry indicator when retry_count is 0", () => {
      const node = makeNode({ id: "t1", retry_count: 0, max_retries: 3 });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-retry").exists()).toBe(false);
    });
  });

  describe("running text", () => {
    it("shows running... text when task state is Running", () => {
      const node = makeNode({ id: "t1", state: "Running" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-running").text()).toBe("running...");
    });

    it("does not show running... text for non-Running state", () => {
      const node = makeNode({ id: "t1", state: "Completed" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-running").exists()).toBe(false);
    });
  });

  describe("action buttons", () => {
    it("shows retry and cancel buttons for Failed tasks", () => {
      const node = makeNode({ id: "t1", state: "Failed" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".btn-retry").exists()).toBe(true);
      expect(wrapper.find(".btn-cancel").exists()).toBe(true);
    });

    it("shows cancel button for Blocked tasks", () => {
      const node = makeNode({ id: "t1", state: "Blocked" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      // Blocked tasks have a cancel button but no retry button
      const cancelButtons = wrapper.findAll(".btn-cancel");
      expect(cancelButtons.length).toBeGreaterThanOrEqual(1);
      expect(wrapper.find(".btn-retry").exists()).toBe(false);
    });

    it("does not show action buttons for Completed tasks", () => {
      const node = makeNode({ id: "t1", state: "Completed" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-actions").exists()).toBe(false);
    });
  });

  describe("error display", () => {
    it("displays error message when task has error", () => {
      const node = makeNode({
        id: "t1",
        state: "Failed",
        error: "Something went wrong"
      });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-error").exists()).toBe(true);
      expect(wrapper.find(".task-error-text").text()).toBe("Something went wrong");
    });

    it("does not display error when error is null", () => {
      const node = makeNode({ id: "t1", state: "Failed", error: null });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-error").exists()).toBe(false);
    });
  });

  describe("expand/collapse", () => {
    it("emits toggle-expand event when clicked and has children", async () => {
      const child = makeNode({ id: "child1" });
      const node = makeNode({ id: "parent" }, [child]);
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      await wrapper.find(".task-node").trigger("click");
      expect(wrapper.emitted("toggle-expand")).toBeTruthy();
      expect(wrapper.emitted("toggle-expand")![0]).toEqual(["parent"]);
    });

    it("renders child nodes when expanded", () => {
      const child = makeNode({ id: "child1", title: "Child Task" });
      const node = makeNode({ id: "parent" }, [child]);
      const expanded = new Set(["parent"]);
      const wrapper = mount(TaskNode, {
        props: { node, expanded, depth: 0 }
      });
      expect(wrapper.find(".task-children").exists()).toBe(true);
      // The child node should be rendered
      const childNodes = wrapper.findAllComponents({ name: "TaskNode" });
      expect(childNodes.length).toBeGreaterThanOrEqual(1);
    });

    it("shows child summary when collapsed with children", () => {
      const child1 = makeNode({ id: "c1", state: "Pending" });
      const child2 = makeNode({ id: "c2", state: "Completed" });
      const node = makeNode({ id: "parent" }, [child1, child2]);
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      const summary = wrapper.find(".task-summary");
      expect(summary.exists()).toBe(true);
      // Should contain icons for each child state
      expect(summary.text()).toContain("⏳");
      expect(summary.text()).toContain("✅");
    });
  });

  describe("button actions", () => {
    it("calls retryTask when retry button is clicked", async () => {
      const node = makeNode({ id: "t1", state: "Failed" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      await wrapper.find(".btn-retry").trigger("click");
      expect(mockRetryTask).toHaveBeenCalledWith("t1");
    });

    it("calls cancelTask when cancel button is clicked on Failed task", async () => {
      const node = makeNode({ id: "t1", state: "Failed" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      await wrapper.find(".btn-cancel").trigger("click");
      expect(mockCancelTask).toHaveBeenCalledWith("t1");
    });

    it("calls cancelTask when cancel button is clicked on Blocked task", async () => {
      const node = makeNode({ id: "t1", state: "Blocked" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      await wrapper.find(".btn-cancel").trigger("click");
      expect(mockCancelTask).toHaveBeenCalledWith("t1");
    });
  });

  describe("depth indentation", () => {
    it("renders with depth indentation for nested nodes", () => {
      const node = makeNode({ id: "child1" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 2 }
      });
      // At depth > 0, the task-indent span should be visible
      const indent = wrapper.find(".task-indent");
      expect(indent.exists()).toBe(true);
    });

    it("does not render indent at depth 0", () => {
      const node = makeNode({ id: "root" });
      const wrapper = mount(TaskNode, {
        props: { node, expanded: new Set(), depth: 0 }
      });
      expect(wrapper.find(".task-indent").exists()).toBe(false);
    });
  });

  it("audit anchors: exposes stable task node pilot selectors", () => {
    const node = makeNode({ id: "audit-task", state: "Running" });
    const wrapper = mount(TaskNode, {
      props: { node, expanded: new Set(), depth: 0 }
    });

    expect(wrapper.find('[data-test="task-node"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="task-node-status"]').exists()).toBe(true);
  });
});

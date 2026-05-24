import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import TaskSteps from "./TaskSteps.vue";
import { useTaskGraphStore } from "@/stores/taskGraph";
import type { TaskSnapshot } from "../types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

const makeTask = (id: string, overrides?: Partial<TaskSnapshot>): TaskSnapshot => ({
  id,
  title: `Task ${id}`,
  role: "Worker",
  state: "Pending",
  dependencies: [],
  error: null,
  ...overrides
});

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("TaskSteps", () => {
  it("shows empty hint when no tasks", () => {
    const wrapper = mount(TaskSteps);
    expect(wrapper.text()).toContain("No tasks yet");
  });

  it("renders task tree with root task", () => {
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph([makeTask("A", { title: "Root Task", state: "Running" })], "ses_1");
    const wrapper = mount(TaskSteps);
    expect(wrapper.text()).toContain("Root Task");
    expect(wrapper.text()).toContain("running...");
  });

  it("renders task state filter chips with live counts", () => {
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph(
      [
        makeTask("active-1", { title: "Queued Task", state: "Pending" }),
        makeTask("active-2", { title: "Running Task", state: "Running" }),
        makeTask("failed-1", { title: "Failed Task", state: "Failed" }),
        makeTask("done-1", { title: "Done Task", state: "Completed" })
      ],
      "ses_1"
    );

    const wrapper = mount(TaskSteps);

    expect(wrapper.find('[data-test="task-state-filters"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="task-filter-all"]').text()).toBe("All 4");
    expect(wrapper.find('[data-test="task-filter-active"]').text()).toBe("Active 2");
    expect(wrapper.find('[data-test="task-filter-failed"]').text()).toBe("Failed 1");
    expect(wrapper.find('[data-test="task-filter-done"]').text()).toBe("Done 1");
  });

  it("filters visible tasks by selected state group", async () => {
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph(
      [
        makeTask("active-1", { title: "Queued Task", state: "Pending" }),
        makeTask("failed-1", { title: "Failed Task", state: "Failed" }),
        makeTask("done-1", { title: "Done Task", state: "Completed" })
      ],
      "ses_1"
    );

    const wrapper = mount(TaskSteps);

    await wrapper.find('[data-test="task-filter-failed"]').trigger("click");

    expect(wrapper.find('[data-test="task-filter-failed"]').attributes("aria-pressed")).toBe(
      "true"
    );
    expect(wrapper.text()).toContain("Failed Task");
    expect(wrapper.text()).not.toContain("Queued Task");
    expect(wrapper.text()).not.toContain("Done Task");
  });

  it("shows error message for failed child task when expanded", async () => {
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph(
      [
        makeTask("parent", { title: "Parent", state: "Completed" }),
        makeTask("child", {
          title: "Child",
          state: "Failed",
          error: "Build failed",
          dependencies: ["parent"]
        })
      ],
      "ses_1"
    );
    const wrapper = mount(TaskSteps);
    // Parent node should be auto-expanded since it has children
    // The error should be present in the DOM
    expect(wrapper.find(".task-error-text").exists()).toBe(true);
    expect(wrapper.find(".task-error-text").text()).toBe("Build failed");
  });

  it("renders state-specific CSS classes", () => {
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph(
      [makeTask("1", { state: "Pending" }), makeTask("2", { state: "Failed", error: "err" })],
      "ses_1"
    );
    const wrapper = mount(TaskSteps);
    expect(wrapper.find(".task-state-pending").exists()).toBe(true);
    expect(wrapper.find(".task-state-failed").exists()).toBe(true);
  });

  it("displays Pending status icon", () => {
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph([makeTask("1", { state: "Pending" })], "ses_1");
    const wrapper = mount(TaskSteps);
    expect(wrapper.find(".task-status").text()).toBe("⏳");
  });

  it("audit anchors: exposes stable task steps pilot selector", () => {
    const wrapper = mount(TaskSteps);

    expect(wrapper.find('[data-test="task-steps"]').exists()).toBe(true);
  });
});

import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import TaskSteps from "./TaskSteps.vue";
import { useTaskGraphStore } from "@/stores/taskGraph";
import type { TaskSnapshot } from "../types";
import en from "@/locales/en.json";
import zhCN from "@/locales/zh-CN.json";
import { mountWithPlugins } from "@/test-utils/mount";

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

function mountTaskSteps(locale: "en" | "zh-CN" = "en") {
  return mountWithPlugins(TaskSteps, { reusePinia: true, locale }).wrapper;
}

describe("TaskSteps", () => {
  it("shows empty hint when no tasks", () => {
    const wrapper = mountTaskSteps();
    expect(wrapper.text()).toContain(en.tasks.empty);
  });

  it("renders localized empty hint in Chinese", () => {
    const wrapper = mountTaskSteps("zh-CN");
    expect(wrapper.text()).toContain(zhCN.tasks.empty);
  });

  it("renders task tree with root task", () => {
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph([makeTask("A", { title: "Root Task", state: "Running" })], "ses_1");
    const wrapper = mountTaskSteps();
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

    const wrapper = mountTaskSteps();

    expect(wrapper.find('[data-test="task-state-filters"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="task-state-filters"]').attributes("aria-label")).toBe(
      en.tasks.stateFiltersAria
    );
    expect(wrapper.find('[data-test="task-filter-all"]').text()).toBe(`${en.tasks.filterAll} 4`);
    expect(wrapper.find('[data-test="task-filter-active"]').text()).toBe(
      `${en.tasks.filterActive} 2`
    );
    expect(wrapper.find('[data-test="task-filter-failed"]').text()).toBe(
      `${en.tasks.filterFailed} 1`
    );
    expect(wrapper.find('[data-test="task-filter-done"]').text()).toBe(`${en.tasks.filterDone} 1`);
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

    const wrapper = mountTaskSteps();

    await wrapper.find('[data-test="task-filter-failed"]').trigger("click");

    expect(wrapper.find('[data-test="task-filter-failed"]').attributes("aria-pressed")).toBe(
      "true"
    );
    expect(wrapper.text()).toContain("Failed Task");
    expect(wrapper.text()).not.toContain("Queued Task");
    expect(wrapper.text()).not.toContain("Done Task");
  });

  it("searches tasks by task text and combines with selected state filter", async () => {
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph(
      [
        makeTask("worker-task-42", {
          title: "Implement Dashboard",
          role: "Worker",
          state: "Running"
        }),
        makeTask("review-task", {
          title: "Review Flow",
          role: "Reviewer",
          state: "Failed",
          error: "Network timeout"
        }),
        makeTask("dependency-match", {
          title: "Blocked Worker",
          state: "Blocked",
          dependencies: ["external-api"]
        }),
        makeTask("done-task", { title: "Archive Docs", state: "Completed" })
      ],
      "ses_1"
    );

    const wrapper = mountTaskSteps();
    const search = wrapper.get('[data-test="task-search-input"]');

    expect(search.attributes("type")).toBe("search");
    expect(search.attributes("aria-label")).toBe(en.tasks.searchAria);
    expect(search.attributes("placeholder")).toBe(en.tasks.searchPlaceholder);

    await search.setValue("dashboard");
    expect(wrapper.text()).toContain("Implement Dashboard");
    expect(wrapper.text()).not.toContain("Review Flow");

    await search.setValue("worker-task-42");
    expect(wrapper.text()).toContain("Implement Dashboard");
    expect(wrapper.text()).not.toContain("Review Flow");

    await search.setValue("Reviewer");
    expect(wrapper.text()).toContain("Review Flow");
    expect(wrapper.text()).not.toContain("Implement Dashboard");

    await search.setValue("Network timeout");
    expect(wrapper.text()).toContain("Review Flow");
    expect(wrapper.text()).not.toContain("Implement Dashboard");

    await search.setValue("external-api");
    expect(wrapper.text()).toContain("Blocked Worker");
    expect(wrapper.text()).not.toContain("Implement Dashboard");

    await wrapper.find('[data-test="task-filter-failed"]').trigger("click");
    await search.setValue("dashboard");
    expect(wrapper.text()).toContain(en.tasks.emptyFiltered);
    expect(wrapper.text()).not.toContain("Implement Dashboard");

    await search.setValue("network");
    expect(wrapper.text()).toContain("Review Flow");
    expect(wrapper.text()).not.toContain("Implement Dashboard");
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
    const wrapper = mountTaskSteps();
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
    const wrapper = mountTaskSteps();
    expect(wrapper.find(".task-state-pending").exists()).toBe(true);
    expect(wrapper.find(".task-state-failed").exists()).toBe(true);
  });

  it("displays Pending status icon", () => {
    const taskGraph = useTaskGraphStore();
    taskGraph.setTaskGraph([makeTask("1", { state: "Pending" })], "ses_1");
    const wrapper = mountTaskSteps();
    expect(wrapper.find(".task-status").text()).toBe("⏳");
  });

  it("audit anchors: exposes stable task steps pilot selector", () => {
    const wrapper = mountTaskSteps();

    expect(wrapper.find('[data-test="task-steps"]').exists()).toBe(true);
  });
});

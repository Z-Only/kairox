import { describe, it, expect, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn()
}));
vi.mock("../composables/useTraceStore", () => ({
  applyTraceEvent: vi.fn(),
  clearTrace: vi.fn()
}));

import { useProjectStore } from "@/stores/project";
import {
  installSidebarTestEnv,
  mockedInvoke,
  mockInvokeCommandResponses,
  mountSidebar
} from "./SessionsSidebar.test-utils";

installSidebarTestEnv();

describe("SessionsSidebar", () => {
  it("imports an existing project from the selected directory", async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(open).mockResolvedValue("/tmp/existing-project");
    mockInvokeCommandResponses({
      add_existing_project: {
        project_id: "project-imported",
        display_name: "existing-project",
        root_path: "/tmp/existing-project",
        removed_at: null,
        sort_order: 0,
        expanded: false
      }
    });

    const { wrapper } = await mountSidebar();
    await wrapper.find('[data-test="project-create-trigger"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="project-import-folder"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("add_existing_project", {
      path: "/tmp/existing-project"
    });
  });

  it("requires a second click on the same project delete button before removing", async () => {
    mockInvokeCommandResponses({
      list_projects: [
        {
          project_id: "project-1",
          display_name: "Demo",
          root_path: "/tmp/demo",
          removed_at: null,
          sort_order: 0,
          expanded: false
        }
      ]
    });
    const { wrapper } = await mountSidebar();
    await flushPromises();

    await wrapper.find('[data-test="project-delete-btn"]').trigger("click");
    await flushPromises();
    expect(mockedInvoke).not.toHaveBeenCalledWith("remove_project", { projectId: "project-1" });
    expect(wrapper.find('[data-test="project-delete-confirm"]').exists()).toBe(true);

    await wrapper.find('[data-test="project-delete-confirm"]').trigger("click");
    await flushPromises();
    expect(mockedInvoke).toHaveBeenCalledWith("remove_project", { projectId: "project-1" });
  });

  it("opens the project create menu without creating a project", async () => {
    const { wrapper } = await mountSidebar();
    const projectStore = useProjectStore();
    const createBlankProject = vi.spyOn(projectStore, "createBlankProject");
    await flushPromises();

    expect(wrapper.find('[data-test="project-create-menu"]').exists()).toBe(false);

    await wrapper.find('[data-test="project-create-trigger"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="project-create-blank"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="project-import-folder"]').exists()).toBe(true);
    expect(createBlankProject).not.toHaveBeenCalled();
  });

  it("opens an inline project rename input from the project action", async () => {
    mockInvokeCommandResponses({
      list_projects: [
        {
          project_id: "project-1",
          display_name: "Demo",
          root_path: "/tmp/demo",
          removed_at: null,
          sort_order: 0,
          expanded: false
        }
      ]
    });
    const { wrapper } = await mountSidebar();
    await flushPromises();

    await wrapper.find('[data-test="project-rename-action-project-1"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="project-rename-input-project-1"]').exists()).toBe(true);
  });
});

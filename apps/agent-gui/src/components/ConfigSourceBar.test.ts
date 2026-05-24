import { flushPromises } from "@vue/test-utils";
import { invoke } from "@tauri-apps/api/core";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ConfigSourceBar from "./ConfigSourceBar.vue";
import { useProjectStore } from "@/stores/project";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

const mockedInvoke = vi.mocked(invoke);

interface ProjectResponse {
  project_id: string;
  display_name: string;
  root_path: string;
  removed_at: string | null;
  sort_order: number;
  expanded: boolean;
  path_exists: boolean;
}

function projectResponse(overrides: Partial<ProjectResponse> = {}): ProjectResponse {
  return {
    project_id: "project-1",
    display_name: "Pilot Project",
    root_path: "/tmp/pilot",
    removed_at: null,
    sort_order: 0,
    expanded: true,
    path_exists: true,
    ...overrides
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("ConfigSourceBar", () => {
  it("emits the selected project when projects load after Project is selected", async () => {
    let resolveProjects!: (projects: ProjectResponse[]) => void;
    mockedInvoke.mockImplementation((command: string) => {
      if (command === "list_projects") {
        return new Promise((resolve) => {
          resolveProjects = resolve;
        });
      }
      return Promise.resolve(null);
    });

    const { wrapper } = mountWithPlugins(ConfigSourceBar, { reusePinia: true });

    await wrapper.get("[data-test='source-btn-project']").trigger("click");
    expect(wrapper.emitted("source-change")?.at(-1)).toEqual(["project", undefined]);

    resolveProjects([projectResponse()]);
    await flushPromises();

    expect(wrapper.emitted("source-change")?.at(-1)).toEqual(["project", "project-1"]);
    expect(wrapper.get<HTMLSelectElement>("select").element.value).toBe("project-1");
  });

  it("renders a warning banner when an active project path is missing", async () => {
    mockedInvoke.mockResolvedValue([
      projectResponse({
        path_exists: false
      })
    ]);

    const { wrapper } = mountWithPlugins(ConfigSourceBar, { reusePinia: true });
    await flushPromises();

    expect(wrapper.find("[data-test='path-warning-banner']").exists()).toBe(true);
  });

  it("falls back to the first available project when the selected project disappears", async () => {
    const projectOne = projectResponse({
      project_id: "project-1",
      display_name: "Alpha Project",
      root_path: "/tmp/alpha",
      sort_order: 0
    });
    const projectTwo = projectResponse({
      project_id: "project-2",
      display_name: "Beta Project",
      root_path: "/tmp/beta",
      sort_order: 1
    });
    mockedInvoke
      .mockResolvedValueOnce([projectOne, projectTwo])
      .mockResolvedValueOnce([projectOne]);

    const projectStore = useProjectStore();
    const { wrapper } = mountWithPlugins(ConfigSourceBar, { reusePinia: true });
    await flushPromises();

    await wrapper.get("[data-test='source-btn-project']").trigger("click");
    await wrapper.get("[data-test='project-select']").setValue("project-2");
    expect(wrapper.emitted("source-change")?.at(-1)).toEqual(["project", "project-2"]);

    await projectStore.loadProjects();
    await flushPromises();

    expect(wrapper.emitted("source-change")?.at(-1)).toEqual(["project", "project-1"]);
    expect(wrapper.get<HTMLSelectElement>("[data-test='project-select']").element.value).toBe(
      "project-1"
    );
  });
});

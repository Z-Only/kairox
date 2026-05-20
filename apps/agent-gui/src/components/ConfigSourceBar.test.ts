import { flushPromises } from "@vue/test-utils";
import { invoke } from "@tauri-apps/api/core";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ConfigSourceBar from "./ConfigSourceBar.vue";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

const mockedInvoke = vi.mocked(invoke);

function projectResponse() {
  return [
    {
      project_id: "project-1",
      display_name: "Pilot Project",
      root_path: "/tmp/pilot",
      removed_at: null,
      sort_order: 0,
      expanded: true
    }
  ];
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("ConfigSourceBar", () => {
  it("emits the selected project when projects load after Project is selected", async () => {
    let resolveProjects!: (projects: ReturnType<typeof projectResponse>) => void;
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

    resolveProjects(projectResponse());
    await flushPromises();

    expect(wrapper.emitted("source-change")?.at(-1)).toEqual(["project", "project-1"]);
    expect(wrapper.get<HTMLSelectElement>("select").element.value).toBe("project-1");
  });
});

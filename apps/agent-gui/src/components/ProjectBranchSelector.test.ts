import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import { useProjectStore } from "@/stores/project";
import { useSessionStore } from "@/stores/session";
import ProjectBranchSelector from "./ProjectBranchSelector.vue";

function mountSelector(props: { projectId?: string; branch?: string | null } = {}) {
  return mountWithPlugins(ProjectBranchSelector, {
    reusePinia: true,
    mount: {
      props: {
        projectId: props.projectId ?? "project_1",
        branch: props.branch ?? null
      }
    }
  }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("ProjectBranchSelector", () => {
  it("renders the root element with data-test attribute", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main"]);
    const wrapper = mountSelector();
    await flushPromises();

    expect(wrapper.find('[data-test="project-branch-selector"]').exists()).toBe(true);
  });

  it("shows the active branch label from props", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main", "dev"]);
    const wrapper = mountSelector({ branch: "dev" });
    await flushPromises();

    expect(wrapper.find('[data-test="session-git-meta"]').text()).toBe("dev");
  });

  it("falls back to first loaded branch when branch prop is null", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main", "dev"]);
    const wrapper = mountSelector({ branch: null });
    await flushPromises();

    expect(wrapper.find('[data-test="session-git-meta"]').text()).toBe("main");
  });

  it("falls back to 'branch' when branch prop is null and no branches loaded", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue([]);
    const wrapper = mountSelector({ branch: null });
    await flushPromises();

    expect(wrapper.find('[data-test="session-git-meta"]').text()).toBe("branch");
  });

  it("loads branches on mount", async () => {
    const projectStore = useProjectStore();
    const spy = vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main"]);
    mountSelector({ projectId: "proj_42" });
    await flushPromises();

    expect(spy).toHaveBeenCalledWith("proj_42");
  });

  it("sets pending branch to first branch when branch prop is null", async () => {
    const projectStore = useProjectStore();
    const session = useSessionStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main", "dev"]);
    const setPendingSpy = vi.spyOn(session, "setPendingProjectBranch");
    mountSelector({ branch: null });
    await flushPromises();

    expect(setPendingSpy).toHaveBeenCalledWith("main");
  });

  it("does not set pending branch when branch prop is already provided", async () => {
    const projectStore = useProjectStore();
    const session = useSessionStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main", "dev"]);
    const setPendingSpy = vi.spyOn(session, "setPendingProjectBranch");
    mountSelector({ branch: "dev" });
    await flushPromises();

    expect(setPendingSpy).not.toHaveBeenCalled();
  });

  it("toggles the popover on button click", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main"]);
    const wrapper = mountSelector();
    await flushPromises();

    expect(wrapper.find('[data-test="project-branch-popover"]').exists()).toBe(false);

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="project-branch-popover"]').exists()).toBe(true);

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");

    expect(wrapper.find('[data-test="project-branch-popover"]').exists()).toBe(false);
  });

  it("renders branch options in the popover", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main", "feat/chat", "dev"]);
    const wrapper = mountSelector();
    await flushPromises();

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="project-branch-option-main"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="project-branch-option-feat-chat"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="project-branch-option-dev"]').exists()).toBe(true);
  });

  it("highlights the active branch option", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main", "dev"]);
    const wrapper = mountSelector({ branch: "dev" });
    await flushPromises();

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="project-branch-option-dev"]').classes()).toContain("active");
    expect(wrapper.find('[data-test="project-branch-option-main"]').classes()).not.toContain(
      "active"
    );
  });

  it("filters branches by search input", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue([
      "main",
      "feat/chat",
      "feat/auth",
      "dev"
    ]);
    const wrapper = mountSelector();
    await flushPromises();

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="project-branch-search"]').setValue("feat");
    await flushPromises();

    expect(wrapper.find('[data-test="project-branch-option-feat-chat"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="project-branch-option-feat-auth"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="project-branch-option-main"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="project-branch-option-dev"]').exists()).toBe(false);
  });

  it("performs case-insensitive search", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["Main", "DEVELOP"]);
    const wrapper = mountSelector();
    await flushPromises();

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="project-branch-search"]').setValue("main");
    await flushPromises();

    expect(wrapper.find('[data-test="project-branch-option-Main"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="project-branch-option-DEVELOP"]').exists()).toBe(false);
  });

  it("selects a branch and closes the popover", async () => {
    const projectStore = useProjectStore();
    const session = useSessionStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main", "dev"]);
    const setPendingSpy = vi.spyOn(session, "setPendingProjectBranch");
    const wrapper = mountSelector();
    await flushPromises();

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="project-branch-option-dev"]').trigger("click");

    expect(setPendingSpy).toHaveBeenCalledWith("dev");
    expect(wrapper.find('[data-test="project-branch-popover"]').exists()).toBe(false);
  });

  it("shows a create option when search does not match any existing branch", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main"]);
    const wrapper = mountSelector();
    await flushPromises();

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="project-branch-search"]').setValue("feat/new-feature");
    await flushPromises();

    const createBtn = wrapper.find('[data-test="project-branch-create"]');
    expect(createBtn.exists()).toBe(true);
    expect(createBtn.text()).toContain("Create feat/new-feature");
  });

  it("does not show create option when search matches an existing branch exactly", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main", "dev"]);
    const wrapper = mountSelector();
    await flushPromises();

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="project-branch-search"]').setValue("main");
    await flushPromises();

    expect(wrapper.find('[data-test="project-branch-create"]').exists()).toBe(false);
  });

  it("does not show create option when search is empty", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main"]);
    const wrapper = mountSelector();
    await flushPromises();

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="project-branch-create"]').exists()).toBe(false);
  });

  it("creates a new branch via the create option", async () => {
    const projectStore = useProjectStore();
    const session = useSessionStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main"]);
    const setPendingSpy = vi.spyOn(session, "setPendingProjectBranch");
    const wrapper = mountSelector();
    await flushPromises();

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="project-branch-search"]').setValue("feat/new-branch");
    await flushPromises();
    await wrapper.find('[data-test="project-branch-create"]').trigger("click");

    expect(setPendingSpy).toHaveBeenCalledWith("feat/new-branch");
    expect(wrapper.find('[data-test="project-branch-popover"]').exists()).toBe(false);
  });

  it("has an accessible search input with aria-label", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main"]);
    const wrapper = mountSelector();
    await flushPromises();

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();

    const searchInput = wrapper.find('[data-test="project-branch-search"]');
    expect(searchInput.exists()).toBe(true);
    // KxInput wraps the native input; check that aria-label is passed
    expect(searchInput.attributes("aria-label")).toBe("Search branches");
  });

  it("handles branch load failure gracefully", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockRejectedValue(new Error("network error"));
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const wrapper = mountSelector();
    await flushPromises();

    expect(consoleSpy).toHaveBeenCalledWith("Failed to load project branches:", expect.any(Error));
    // Should show fallback label
    expect(wrapper.find('[data-test="session-git-meta"]').text()).toBe("branch");
    consoleSpy.mockRestore();
  });

  it("uses button type='button' for the toggle and branch options", async () => {
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main"]);
    const wrapper = mountSelector();
    await flushPromises();

    expect(wrapper.find('[data-test="session-git-meta"]').attributes("type")).toBe("button");

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="project-branch-option-main"]').attributes("type")).toBe(
      "button"
    );
  });
});

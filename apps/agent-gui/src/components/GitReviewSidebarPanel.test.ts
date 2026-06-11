import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import GitReviewSidebarPanel from "./GitReviewSidebarPanel.vue";
import { useWorkspaceUiStore } from "@/stores/workspaceUi";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockedInvoke = vi.mocked(invoke);

function mountPanel() {
  return mountWithPlugins(GitReviewSidebarPanel, { reusePinia: true }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("GitReviewSidebarPanel", () => {
  it("renders loading state and disables refresh", () => {
    const workspaceUi = useWorkspaceUiStore();
    workspaceUi.gitReviewLoading = true;

    const wrapper = mountPanel();

    expect(wrapper.get('[data-test="git-review-loading"]').text()).toBe("Loading changes…");
    expect(wrapper.get('[data-test="git-review-refresh"]').attributes("disabled")).toBeDefined();
  });

  it("renders error and empty states", () => {
    const workspaceUi = useWorkspaceUiStore();
    workspaceUi.gitReviewError = "git failed";

    const errorWrapper = mountPanel();
    expect(errorWrapper.get('[data-test="git-review-error"]').text()).toContain("git failed");

    workspaceUi.gitReviewError = null;
    const emptyWrapper = mountPanel();
    expect(emptyWrapper.get('[data-test="git-review-empty"]').text()).toBe(
      "Open repository changes from a project chat."
    );
  });

  it("renders clean repository state without diff sections", () => {
    const workspaceUi = useWorkspaceUiStore();
    workspaceUi.gitReview = {
      kind: "clean",
      branch: null,
      worktreePath: "/repo",
      message: null,
      changedFiles: [],
      staged: null,
      unstaged: null,
      untracked: null
    };

    const wrapper = mountPanel();

    expect(wrapper.find('[data-test="git-review-branch"]').exists()).toBe(false);
    expect(wrapper.get('[data-test="git-review-clean"]').text()).toBe("No local changes");
    expect(wrapper.find('[data-test="git-review-section"]').exists()).toBe(false);
  });

  it("refreshes project review from the sidebar button", async () => {
    mockedInvoke.mockResolvedValueOnce({
      kind: "dirty",
      branch: "feat/review",
      worktree_path: "/repo",
      message: null,
      changed_files: ["README.md"],
      staged: null,
      unstaged: null,
      untracked: null
    });
    const workspaceUi = useWorkspaceUiStore();
    workspaceUi.gitReviewContext = { projectId: "project_1" };
    const wrapper = mountPanel();

    await wrapper.get('[data-test="git-review-refresh"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("get_project_git_review", {
      projectId: "project_1"
    });
    expect(workspaceUi.gitReview?.changedFiles).toEqual(["README.md"]);
    expect(wrapper.get('[data-test="git-review-files"]').text()).toContain("README.md");
  });
});

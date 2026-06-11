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
      fileCount: 0,
      additions: 0,
      deletions: 0,
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

  it("renders file count, line stats, collapsible file diffs, and collapsed context", async () => {
    const workspaceUi = useWorkspaceUiStore();
    workspaceUi.gitReview = {
      kind: "dirty",
      branch: "feat/review",
      worktreePath: "/repo",
      message: null,
      fileCount: 2,
      additions: 3,
      deletions: 1,
      changedFiles: ["src/App.vue", "notes.txt"],
      staged: null,
      unstaged: {
        label: "Unstaged changes",
        stat: " src/App.vue | 2 +-\n notes.txt | 1 +",
        additions: 3,
        deletions: 1,
        files: [
          {
            path: "src/App.vue",
            additions: 2,
            deletions: 1,
            diff: "--- a/src/App.vue\n+++ b/src/App.vue\n@@ -1,4 +1,4 @@\n keep\n-old\n+new\n+extra"
          },
          {
            path: "notes.txt",
            additions: 1,
            deletions: 0,
            diff: "diff --git a/notes.txt b/notes.txt\n+++ b/notes.txt\n+draft"
          }
        ],
        diff: "--- a/src/App.vue\n+++ b/src/App.vue\n@@ -1,4 +1,4 @@\n keep\n-old\n+new\n+extra"
      },
      untracked: null
    };

    const wrapper = mountPanel();

    expect(wrapper.get('[data-test="git-review-summary"]').text()).toContain("2 files");
    expect(wrapper.get('[data-test="git-review-summary"]').text()).toContain("+3 -1");
    expect(wrapper.get('[data-test="git-review-files"]').text()).toContain("src/App.vue");
    expect(wrapper.get('[data-test="git-review-files"]').text()).toContain("+2 -1");
    expect(wrapper.get('[data-test="git-review-file-change"]').text()).toContain("src/App.vue");
    expect(wrapper.get('[data-test="git-review-file-change"]').text()).toContain("+2 -1");
    expect(wrapper.get('[data-test="git-review-file-toggle"]').attributes("aria-label")).toBe(
      "Toggle diff for src/App.vue"
    );
    expect(wrapper.text()).toContain("-old");
    expect(wrapper.text()).toContain("+new");
    expect(wrapper.text()).not.toContain(" keep");
    expect(wrapper.get('[data-test="diff-collapsed-context"]').text()).toBe(
      "Show 1 unchanged line"
    );

    await wrapper.get('[data-test="diff-collapsed-context"]').trigger("click");
    expect(wrapper.text()).toContain(" keep");

    const firstFile = wrapper.findAll('[data-test="git-review-file-change"]')[0];
    await firstFile.get('[data-test="git-review-file-toggle"]').trigger("click");
    expect(firstFile.find('[data-test="git-review-file-diff"]').exists()).toBe(false);

    await firstFile.get('[data-test="git-review-file-toggle"]').trigger("click");
    expect(firstFile.find('[data-test="git-review-file-diff"]').exists()).toBe(true);
  });

  it("refreshes project review from the sidebar button", async () => {
    mockedInvoke.mockResolvedValueOnce({
      kind: "dirty",
      branch: "feat/review",
      worktree_path: "/repo",
      message: null,
      file_count: 1,
      additions: 1,
      deletions: 0,
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

import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useWorkspaceUiStore } from "@/stores/workspaceUi";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("useWorkspaceUiStore", () => {
  it("starts with the default section order and a closed archive", () => {
    const store = useWorkspaceUiStore();
    expect(store.sectionOrder).toEqual(["projects", "sessions"]);
    expect(store.archiveOpen).toBe(false);
  });

  it("accepts subagents as a right panel tab", () => {
    const store = useWorkspaceUiStore();
    store.setRightPanelTab("subagents");
    expect(store.rightPanelTab).toBe("subagents");
  });

  it("moveSectionUp returns early when the section is already at the top", () => {
    const store = useWorkspaceUiStore();
    store.moveSectionUp("projects");
    expect(store.sectionOrder).toEqual(["projects", "sessions"]);
  });

  it("moveSectionUp returns early when the section is not in the order", () => {
    const store = useWorkspaceUiStore();
    store.moveSectionUp("unknown" as "projects");
    expect(store.sectionOrder).toEqual(["projects", "sessions"]);
  });

  it("moveSectionUp swaps the section with the one above it", () => {
    const store = useWorkspaceUiStore();
    store.moveSectionUp("sessions");
    expect(store.sectionOrder).toEqual(["sessions", "projects"]);
  });

  it("opens git review in the changes panel", async () => {
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
      unstaged: {
        label: "Unstaged changes",
        stat: " README.md | 1 +",
        diff: "+local agent edit",
        additions: 1,
        deletions: 0,
        files: [
          {
            path: "README.md",
            additions: 1,
            deletions: 0,
            diff: "+local agent edit"
          }
        ]
      },
      untracked: null
    });
    const store = useWorkspaceUiStore();

    await store.openGitReview({ sessionId: "ses_1", projectId: "project_1" });

    expect(store.rightPanelTab).toBe("changes");
    expect(mockedInvoke).toHaveBeenCalledWith("get_session_git_review", {
      sessionId: "ses_1"
    });
    expect(store.gitReview?.changedFiles).toEqual(["README.md"]);
    expect(store.gitReviewError).toBeNull();
  });

  it("falls back to project git review when the draft session is not bound", async () => {
    mockedInvoke
      .mockRejectedValueOnce(new Error("invalid state: session is not bound to a project"))
      .mockResolvedValueOnce({
        kind: "dirty",
        branch: "main",
        worktree_path: "/repo",
        message: null,
        file_count: 1,
        additions: 1,
        deletions: 0,
        changed_files: ["VIBE_REVIEW_NOTES.md"],
        staged: null,
        unstaged: null,
        untracked: {
          label: "Untracked files",
          stat: " VIBE_REVIEW_NOTES.md | 1 +",
          diff: "+new file from simulated agent",
          additions: 1,
          deletions: 0,
          files: [
            {
              path: "VIBE_REVIEW_NOTES.md",
              additions: 1,
              deletions: 0,
              diff: "+new file from simulated agent"
            }
          ]
        }
      });
    const store = useWorkspaceUiStore();

    await store.openGitReview({
      sessionId: "ses_draft",
      projectId: "project_1"
    });

    expect(mockedInvoke).toHaveBeenCalledWith("get_session_git_review", {
      sessionId: "ses_draft"
    });
    expect(mockedInvoke).toHaveBeenCalledWith("get_project_git_review", {
      projectId: "project_1"
    });
    expect(store.gitReview?.changedFiles).toEqual(["VIBE_REVIEW_NOTES.md"]);
    expect(store.gitReviewError).toBeNull();
  });

  it("clears cached git review data", () => {
    const store = useWorkspaceUiStore();
    store.rightPanelTab = "changes";
    store.gitReviewContext = { sessionId: "ses_1", projectId: "project_1" };
    store.gitReview = {
      kind: "dirty",
      branch: "main",
      worktreePath: "/repo",
      message: null,
      fileCount: 1,
      additions: 1,
      deletions: 0,
      changedFiles: ["README.md"],
      staged: null,
      unstaged: null,
      untracked: null
    };
    store.gitReviewLoading = true;
    store.gitReviewError = "stale error";

    store.clearGitReview();

    expect(store.gitReviewContext).toBeNull();
    expect(store.gitReview).toBeNull();
    expect(store.gitReviewLoading).toBe(false);
    expect(store.gitReviewError).toBeNull();
    expect(store.rightPanelTab).toBe("changes");
  });
});

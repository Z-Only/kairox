import { setActivePinia, createPinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useProjectStore } from "./project";
import { useSessionStore } from "./session";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

const mockedInvoke = vi.mocked(invoke);

function mockDefaultInvoke(): void {
  mockedInvoke.mockImplementation(
    async (
      command: string,
      args: { displayName?: string | null; display_name?: string | null } = {}
    ) => {
      if (command === "list_projects") {
        return [
          {
            project_id: "p1",
            display_name: "Demo",
            root_path: "/tmp/demo",
            removed_at: null,
            sort_order: 0,
            expanded: true
          }
        ];
      }
      if (command === "create_blank_project") {
        const displayName = args.displayName ?? args.display_name ?? "New Project";
        return {
          project_id: "p2",
          display_name: displayName,
          root_path: "/tmp/scratch",
          removed_at: null,
          sort_order: 1,
          expanded: true
        };
      }
      if (command === "list_project_sessions") {
        return [
          {
            id: "s1",
            title: "Draft",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/demo",
            branch: null,
            deleted_at: null,
            visibility: "draft_hidden"
          }
        ];
      }
      if (command === "list_archived_sessions") {
        return [
          {
            id: "s2",
            title: "Archived",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/demo",
            branch: "main",
            deleted_at: "2026-01-02T03:04:05Z",
            visibility: "archived"
          }
        ];
      }
      if (command === "get_project_git_status") {
        return {
          kind: "Clean",
          branch: "main",
          worktree_path: "/tmp/demo",
          message: null
        };
      }
      if (command === "get_project_instruction_summary") {
        return {
          source_paths: ["/tmp/demo/AGENTS.md"],
          warning: null
        };
      }
      return null;
    }
  );
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockDefaultInvoke();
});

describe("project store", () => {
  it("loads active projects into normalized state", async () => {
    const store = useProjectStore();

    await store.loadProjects();

    expect(store.projects).toHaveLength(1);
    expect(store.projects[0].displayName).toBe("Demo");
    expect(store.activeProjects).toHaveLength(1);
  });

  it("creates blank projects and appends them to local state", async () => {
    const store = useProjectStore();

    const project = await store.createBlankProject("Scratch");

    expect(mockedInvoke).toHaveBeenCalledWith("create_blank_project", {
      displayName: "Scratch"
    });
    expect(project.projectId).toBe("p2");
    expect(store.projects.map((entry) => entry.projectId)).toEqual(["p2"]);
  });

  it("uses New Project as the default blank project name", async () => {
    const store = useProjectStore();

    const project = await store.createBlankProject();

    expect(mockedInvoke).toHaveBeenCalledWith("create_blank_project", {
      displayName: null
    });
    expect(project.displayName).toBe("New Project");
  });

  it("refreshes projects from backend after removing a project", async () => {
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === "remove_project") {
        return null;
      }
      if (command === "list_projects") {
        return [
          {
            project_id: "p1",
            display_name: "Demo",
            root_path: "/tmp/demo",
            removed_at: "2026-05-10T00:00:00Z",
            sort_order: 0,
            expanded: true
          }
        ];
      }
      return null;
    });
    const store = useProjectStore();

    await store.removeProject("p1");

    expect(mockedInvoke).toHaveBeenCalledWith("remove_project", {
      projectId: "p1"
    });
    expect(mockedInvoke).toHaveBeenCalledWith("list_projects");
    expect(store.projects[0].removedAt).toBe("2026-05-10T00:00:00Z");
  });

  it("creates a hidden project draft without relying on visible session reload", async () => {
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === "list_projects") {
        return [
          {
            project_id: "p1",
            display_name: "Demo",
            root_path: "/tmp/demo",
            removed_at: null,
            sort_order: 0,
            expanded: true
          }
        ];
      }
      if (command === "create_project_draft_session") {
        return "s-draft";
      }
      if (command === "list_project_sessions") {
        return [
          {
            id: "s-visible",
            title: "Visible",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/demo",
            branch: null,
            deleted_at: null,
            visibility: "visible"
          }
        ];
      }
      return null;
    });
    const store = useProjectStore();
    await store.loadProjects();

    const draftSession = await store.createProjectDraftSession("p1");

    expect(draftSession).toEqual({
      sessionId: "s-draft",
      title: "New Session",
      profile: "default",
      projectId: "p1",
      worktreePath: "/tmp/demo",
      branch: null,
      deletedAt: null,
      visibility: "draft_hidden",
      approvalPolicy: null,
      sandboxPolicy: null
    });
    expect(mockedInvoke).not.toHaveBeenCalledWith("list_project_sessions", {
      projectId: "p1"
    });
    expect(store.sessionsByProject.get("p1")?.map((session) => session.sessionId)).toEqual([
      "s-draft"
    ]);
  });

  it("creates a worktree session with branch name", async () => {
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === "list_projects") {
        return [
          {
            project_id: "p1",
            display_name: "Demo",
            root_path: "/tmp/demo",
            removed_at: null,
            sort_order: 0,
            expanded: true
          }
        ];
      }
      if (command === "create_project_worktree_session") {
        return "s-worktree";
      }
      if (command === "get_session_git_status") {
        return {
          kind: "clean",
          branch: "feat/my-branch",
          worktree_path: "/tmp/demo/.kairox/worktrees/feat-my-branch",
          message: null
        };
      }
      if (command === "rename_session") {
        return null;
      }
      return null;
    });
    const store = useProjectStore();
    await store.loadProjects();

    const worktreeSession = await store.createProjectWorktreeSession("p1", "feat/my-branch");

    expect(mockedInvoke).toHaveBeenCalledWith("create_project_worktree_session", {
      projectId: "p1",
      branchName: "feat/my-branch"
    });
    expect(worktreeSession.sessionId).toBe("s-worktree");
    expect(worktreeSession.title).toBe("New Session (feat/my-branch)");
    expect(worktreeSession.branch).toBe("feat/my-branch");
    expect(worktreeSession.worktreePath).toBe("/tmp/demo/.kairox/worktrees/feat-my-branch");
    expect(worktreeSession.visibility).toBe("visible");
    expect(store.sessionsByProject.get("p1")?.map((s) => s.sessionId)).toEqual(["s-worktree"]);
  });

  it("creates a worktree session with deduped title", async () => {
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === "create_project_worktree_session") {
        return "s-worktree-2";
      }
      if (command === "rename_session") {
        return null;
      }
      return null;
    });
    const store = useProjectStore();
    store.sessionsByProject = new Map([
      [
        "p1",
        [
          {
            sessionId: "s-existing",
            title: "New Session (main)",
            profile: "default",
            projectId: "p1",
            worktreePath: "/tmp/demo",
            branch: "main",
            deletedAt: null,
            visibility: "visible"
          }
        ]
      ]
    ]);

    const session = await store.createProjectWorktreeSession("p1", "main");
    expect(session.title).toBe("New Session (main) 1");
  });

  it("loads project sessions and archived sessions into separate maps", async () => {
    const store = useProjectStore();

    await store.loadProjectSessions("p1");
    await store.loadArchivedSessions();

    expect(store.sessionsByProject.get("p1")?.[0].sessionId).toBe("s1");
    expect(store.archivedSessions[0].sessionId).toBe("s2");
  });

  it("reloads profile info after project config refresh instead of reusing an in-flight cache", async () => {
    const calls: string[] = [];
    let releaseStaleProfiles: (() => void) | null = null;
    const staleProfiles = new Promise((resolve) => {
      releaseStaleProfiles = () =>
        resolve([
          {
            alias: "cached",
            provider: "fake",
            model_id: "fake",
            local: true,
            has_api_key: true
          }
        ]);
    });
    mockedInvoke.mockImplementation(async (command: string) => {
      calls.push(command);
      if (command === "get_profile_info") {
        const count = calls.filter((entry) => entry === "get_profile_info").length;
        if (count === 1) return staleProfiles;
        return [
          {
            alias: "tokensflow",
            provider: "openai_compatible",
            model_id: "tokensflow-chat",
            local: false,
            has_api_key: true
          }
        ];
      }
      if (command === "refresh_config_for_project") return null;
      if (command === "list_project_sessions") return [];
      return null;
    });
    const session = useSessionStore();
    const store = useProjectStore();
    store.projects = [
      {
        projectId: "p1",
        displayName: "Demo",
        rootPath: "/tmp/demo",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];

    const staleLoad = session.loadProfileInfo();
    const projectLoad = store.loadProjectSessions("p1");
    releaseStaleProfiles?.();
    await staleLoad;
    await projectLoad;

    expect(calls.filter((entry) => entry === "get_profile_info")).toHaveLength(2);
    expect(session.profileInfos.map((profile) => profile.alias)).toEqual(["tokensflow"]);
  });

  it("updates nested project sessions through the regular session IPC paths", async () => {
    const store = useProjectStore();
    store.sessionsByProject = new Map([
      [
        "p1",
        [
          {
            sessionId: "s1",
            title: "Draft",
            profile: "fast",
            projectId: "p1",
            worktreePath: "/tmp/demo",
            branch: null,
            deletedAt: null,
            visibility: "visible"
          }
        ]
      ]
    ]);

    await store.renameProjectSession("s1", "Renamed Draft");
    await store.archiveProjectSession("s1");

    expect(mockedInvoke).toHaveBeenCalledWith("rename_session", {
      sessionId: "s1",
      title: "Renamed Draft"
    });
    expect(mockedInvoke).toHaveBeenCalledWith("delete_session", {
      sessionId: "s1"
    });
    expect(store.sessionsByProject.get("p1")?.map((entry) => entry.title)).toEqual([]);
  });

  it("normalizes git status and project instruction summary responses", async () => {
    const store = useProjectStore();

    const gitStatus = await store.getProjectGitStatus("p1");
    const instructionSummary = await store.getProjectInstructionSummary("p1");

    expect(gitStatus).toEqual({
      kind: "Clean",
      branch: "main",
      worktreePath: "/tmp/demo",
      message: null
    });
    expect(instructionSummary.sourcePaths).toEqual(["/tmp/demo/AGENTS.md"]);
  });

  it("lists project branches for the new-session branch picker", async () => {
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === "list_project_branches") return ["main", "feat/chat"];
      return null;
    });
    const store = useProjectStore();

    await expect(store.listProjectBranches("p1")).resolves.toEqual(["main", "feat/chat"]);
    expect(mockedInvoke).toHaveBeenCalledWith("list_project_branches", {
      projectId: "p1"
    });
  });
});

describe("project store — additional coverage", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    mockDefaultInvoke();
  });

  it("addExistingProject adds a project from an existing path", async () => {
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === "add_existing_project") {
        return {
          project_id: "p3",
          display_name: "Existing",
          root_path: "/tmp/existing",
          removed_at: null,
          sort_order: 2,
          expanded: true,
          path_exists: true
        };
      }
      return null;
    });
    const store = useProjectStore();

    const project = await store.addExistingProject("/tmp/existing");

    expect(mockedInvoke).toHaveBeenCalledWith("add_existing_project", {
      path: "/tmp/existing"
    });
    expect(project.projectId).toBe("p3");
    expect(project.displayName).toBe("Existing");
    expect(store.projects).toContainEqual(expect.objectContaining({ projectId: "p3" }));
  });

  it("renameProject updates the local project name without refetching", async () => {
    const store = useProjectStore();
    store.projects = [
      {
        projectId: "p1",
        displayName: "Old Name",
        rootPath: "/tmp/demo",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];

    await store.renameProject("p1", "New Name");

    expect(mockedInvoke).toHaveBeenCalledWith("rename_project", {
      projectId: "p1",
      displayName: "New Name"
    });
    expect(store.projects[0].displayName).toBe("New Name");
  });

  it("renameProject does not change other projects", async () => {
    const store = useProjectStore();
    store.projects = [
      {
        projectId: "p1",
        displayName: "First",
        rootPath: "/tmp/first",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      },
      {
        projectId: "p2",
        displayName: "Second",
        rootPath: "/tmp/second",
        removedAt: null,
        sortOrder: 1,
        expanded: true,
        pathExists: true
      }
    ];

    await store.renameProject("p1", "Renamed");

    expect(store.projects[1].displayName).toBe("Second");
  });

  it("updateProjectOrder re-sorts projects by the new order", async () => {
    const store = useProjectStore();
    store.projects = [
      {
        projectId: "p1",
        displayName: "A",
        rootPath: "/a",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      },
      {
        projectId: "p2",
        displayName: "B",
        rootPath: "/b",
        removedAt: null,
        sortOrder: 1,
        expanded: true,
        pathExists: true
      },
      {
        projectId: "p3",
        displayName: "C",
        rootPath: "/c",
        removedAt: null,
        sortOrder: 2,
        expanded: true,
        pathExists: true
      }
    ];

    await store.updateProjectOrder(["p3", "p1", "p2"]);

    expect(mockedInvoke).toHaveBeenCalledWith("update_project_order", {
      projectIds: ["p3", "p1", "p2"]
    });
    expect(store.projects.map((p) => p.projectId)).toEqual(["p3", "p1", "p2"]);
    expect(store.projects[0].sortOrder).toBe(0);
    expect(store.projects[1].sortOrder).toBe(1);
    expect(store.projects[2].sortOrder).toBe(2);
  });

  it("updateProjectExpanded toggles the expanded flag locally", async () => {
    const store = useProjectStore();
    store.projects = [
      {
        projectId: "p1",
        displayName: "A",
        rootPath: "/a",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];

    await store.updateProjectExpanded("p1", false);

    expect(mockedInvoke).toHaveBeenCalledWith("update_project_expanded", {
      projectId: "p1",
      expanded: false
    });
    expect(store.projects[0].expanded).toBe(false);
  });

  it("refreshProjectConfig calls refreshConfigForProject for known project", async () => {
    const store = useProjectStore();
    store.projects = [
      {
        projectId: "p1",
        displayName: "A",
        rootPath: "/tmp/a",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];

    await store.refreshProjectConfig("p1");

    expect(mockedInvoke).toHaveBeenCalledWith("refresh_config_for_project", {
      projectRoot: "/tmp/a"
    });
  });

  it("refreshProjectConfig does nothing for unknown project", async () => {
    const store = useProjectStore();
    store.projects = [];

    await store.refreshProjectConfig("unknown");

    expect(mockedInvoke).not.toHaveBeenCalledWith("refresh_config_for_project", expect.anything());
  });

  it("refreshProjectConfigRoot calls refreshConfigForProject with given path", async () => {
    const store = useProjectStore();

    await store.refreshProjectConfigRoot("/some/path");

    expect(mockedInvoke).toHaveBeenCalledWith("refresh_config_for_project", {
      projectRoot: "/some/path"
    });
  });

  it("restoreProjectSession normalizes the response and reloads sessions", async () => {
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === "restore_project_session") {
        return {
          project_id: "p1",
          display_name: "Restored",
          root_path: "/tmp/restored",
          removed_at: null,
          sort_order: 0,
          expanded: true,
          path_exists: true
        };
      }
      if (command === "list_project_sessions") {
        return [
          {
            id: "s1",
            title: "Session",
            profile: "default",
            project_id: "p1",
            worktree_path: "/tmp/restored",
            branch: "main",
            deleted_at: null,
            visibility: "visible"
          }
        ];
      }
      return null;
    });
    const store = useProjectStore();

    const project = await store.restoreProjectSession("s1");

    expect(mockedInvoke).toHaveBeenCalledWith("restore_project_session", {
      sessionId: "s1"
    });
    expect(project.projectId).toBe("p1");
    expect(project.displayName).toBe("Restored");
    expect(store.projects).toContainEqual(expect.objectContaining({ projectId: "p1" }));
    expect(store.sessionsByProject.get("p1")).toHaveLength(1);
  });

  it("initProjectGit returns normalized git status", async () => {
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === "init_project_git") {
        return {
          kind: "Initialized",
          branch: "main",
          worktree_path: "/tmp/demo",
          message: "Initialized empty Git repository"
        };
      }
      return null;
    });
    const store = useProjectStore();

    const result = await store.initProjectGit("p1");

    expect(mockedInvoke).toHaveBeenCalledWith("init_project_git", {
      projectId: "p1"
    });
    expect(result).toEqual({
      kind: "Initialized",
      branch: "main",
      worktreePath: "/tmp/demo",
      message: "Initialized empty Git repository"
    });
  });

  it("getSessionGitStatus normalizes the backend response", async () => {
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === "get_session_git_status") {
        return {
          kind: "Dirty",
          branch: "feat/test",
          worktree_path: "/tmp/worktree",
          message: "2 files changed"
        };
      }
      return null;
    });
    const store = useProjectStore();

    const result = await store.getSessionGitStatus("s1");

    expect(mockedInvoke).toHaveBeenCalledWith("get_session_git_status", {
      sessionId: "s1"
    });
    expect(result).toEqual({
      kind: "Dirty",
      branch: "feat/test",
      worktreePath: "/tmp/worktree",
      message: "2 files changed"
    });
  });

  it("getSessionGitReview normalizes changed files and diff sections", async () => {
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === "get_session_git_review") {
        return {
          kind: "dirty",
          branch: "feat/test",
          worktree_path: "/tmp/worktree",
          message: null,
          file_count: 2,
          additions: 2,
          deletions: 0,
          changed_files: ["src/App.vue", "notes.txt"],
          staged: {
            label: "Staged changes",
            stat: " src/App.vue | 1 +",
            diff: "--- a/src/App.vue\n+++ b/src/App.vue\n@@ -1 +1 @@\n-old\n+new",
            additions: 1,
            deletions: 0,
            files: [
              {
                path: "src/App.vue",
                additions: 1,
                deletions: 0,
                diff: "--- a/src/App.vue\n+++ b/src/App.vue\n@@ -1 +1 @@\n-old\n+new"
              }
            ]
          },
          unstaged: null,
          untracked: {
            label: "Untracked files",
            stat: " notes.txt | 1 +",
            diff: "+++ b/notes.txt\n+draft",
            additions: 1,
            deletions: 0,
            files: [
              {
                path: "notes.txt",
                additions: 1,
                deletions: 0,
                diff: "+++ b/notes.txt\n+draft"
              }
            ]
          }
        };
      }
      return null;
    });
    const store = useProjectStore();

    const result = await store.getSessionGitReview("s1");

    expect(mockedInvoke).toHaveBeenCalledWith("get_session_git_review", {
      sessionId: "s1"
    });
    expect(result).toEqual({
      kind: "dirty",
      branch: "feat/test",
      worktreePath: "/tmp/worktree",
      message: null,
      fileCount: 2,
      additions: 2,
      deletions: 0,
      changedFiles: ["src/App.vue", "notes.txt"],
      staged: {
        label: "Staged changes",
        stat: " src/App.vue | 1 +",
        diff: "--- a/src/App.vue\n+++ b/src/App.vue\n@@ -1 +1 @@\n-old\n+new",
        additions: 1,
        deletions: 0,
        files: [
          {
            path: "src/App.vue",
            additions: 1,
            deletions: 0,
            diff: "--- a/src/App.vue\n+++ b/src/App.vue\n@@ -1 +1 @@\n-old\n+new"
          }
        ]
      },
      unstaged: null,
      untracked: {
        label: "Untracked files",
        stat: " notes.txt | 1 +",
        diff: "+++ b/notes.txt\n+draft",
        additions: 1,
        deletions: 0,
        files: [
          {
            path: "notes.txt",
            additions: 1,
            deletions: 0,
            diff: "+++ b/notes.txt\n+draft"
          }
        ]
      }
    });
  });

  it("getSessionGitReview fills defaults for legacy diff metadata", async () => {
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === "get_session_git_review") {
        return {
          kind: "dirty",
          branch: "feat/legacy",
          worktree_path: "/tmp/worktree",
          message: null,
          changed_files: ["README.md"],
          staged: {
            label: "Staged changes",
            stat: " README.md | 1 +",
            diff: "+legacy"
          },
          unstaged: null,
          untracked: null
        };
      }
      return null;
    });
    const store = useProjectStore();

    const result = await store.getSessionGitReview("s1");

    expect(result.fileCount).toBe(1);
    expect(result.additions).toBe(0);
    expect(result.deletions).toBe(0);
    expect(result.staged).toEqual({
      label: "Staged changes",
      stat: " README.md | 1 +",
      diff: "+legacy",
      additions: 0,
      deletions: 0,
      files: []
    });
  });

  it("getProjectInstructionSummary stores result and handles errors gracefully", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("ENOENT"));
    const store = useProjectStore();

    const result = await store.getProjectInstructionSummary("p-missing");

    expect(result.sourcePaths).toEqual([]);
    expect(result.warning).toContain("ENOENT");
    expect(store.instructionSummariesByProject.get("p-missing")).toEqual(result);
  });

  it("activeProjects filters out removed projects", () => {
    const store = useProjectStore();
    store.projects = [
      {
        projectId: "p1",
        displayName: "Active",
        rootPath: "/active",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      },
      {
        projectId: "p2",
        displayName: "Removed",
        rootPath: "/removed",
        removedAt: "2026-01-01T00:00:00Z",
        sortOrder: 1,
        expanded: true,
        pathExists: true
      }
    ];

    expect(store.activeProjects).toHaveLength(1);
    expect(store.activeProjects[0].projectId).toBe("p1");
  });
});

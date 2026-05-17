import { setActivePinia, createPinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useProjectStore } from "./project";

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

    expect(mockedInvoke).toHaveBeenCalledWith("create_blank_project", { displayName: "Scratch" });
    expect(project.projectId).toBe("p2");
    expect(store.projects.map((entry) => entry.projectId)).toEqual(["p2"]);
  });

  it("uses New Project as the default blank project name", async () => {
    const store = useProjectStore();

    const project = await store.createBlankProject();

    expect(mockedInvoke).toHaveBeenCalledWith("create_blank_project", { displayName: null });
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

    expect(mockedInvoke).toHaveBeenCalledWith("remove_project", { projectId: "p1" });
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
      visibility: "draft_hidden"
    });
    expect(mockedInvoke).not.toHaveBeenCalledWith("list_project_sessions", { projectId: "p1" });
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
    expect(mockedInvoke).toHaveBeenCalledWith("delete_session", { sessionId: "s1" });
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
});

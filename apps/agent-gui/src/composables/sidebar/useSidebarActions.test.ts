import { describe, it, expect, vi, beforeEach } from "vitest";
import type { ProjectInfo, ProjectSessionInfo } from "@/stores/project";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockRouterPush = vi.fn();
const mockRouterReplace = vi.fn();
const routeParams: { sessionId?: string | string[] } = { sessionId: "active_1" };

vi.mock("vue-router", () => ({
  useRoute: () => ({ params: routeParams }),
  useRouter: () => ({ push: mockRouterPush, replace: mockRouterReplace })
}));

const sessionStore = {
  currentSessionId: "fallback_1" as string | null,
  currentSessionInfo: null as { project_id?: string | null } | null,
  startOrdinaryDraftSession: vi.fn(),
  deleteSession: vi.fn(),
  switchProjectSession: vi.fn(),
  startProjectDraftSession: vi.fn()
};

vi.mock("@/stores/session", () => ({
  useSessionStore: () => sessionStore
}));

const projectStore = {
  sessionsByProject: new Map<string, ProjectSessionInfo[]>(),
  activeProjects: [] as ProjectInfo[],
  sidebarProjects: [] as ProjectInfo[],
  createBlankProject: vi.fn(),
  addExistingProject: vi.fn(),
  loadProjects: vi.fn(),
  archiveProjectSession: vi.fn(),
  updateProjectExpanded: vi.fn(),
  loadProjectSessions: vi.fn(),
  removeProject: vi.fn()
};

vi.mock("@/stores/project", () => ({
  useProjectStore: () => projectStore
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn()
}));

// Dynamic import so mocks are applied before the module is loaded
const { useSidebarActions } = await import("./useSidebarActions");

// Re-import the mocked `open` so we can control its return value
const { open } = await import("@tauri-apps/plugin-dialog");
const mockOpen = open as ReturnType<typeof vi.fn>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeProjectSession(overrides: Partial<ProjectSessionInfo> = {}): ProjectSessionInfo {
  return {
    sessionId: "ps_1",
    title: "Project Session",
    profile: "default",
    projectId: "proj_1",
    worktreePath: null,
    branch: null,
    visibility: null,
    deletedAt: null,
    ...overrides
  };
}

function makeProject(overrides: Partial<ProjectInfo> = {}): ProjectInfo {
  return {
    projectId: "proj_1",
    displayName: "My Project",
    rootPath: "/tmp/proj",
    removedAt: null,
    sortOrder: 0,
    expanded: false,
    pathExists: true,
    ...overrides
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("useSidebarActions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    routeParams.sessionId = "active_1";
    sessionStore.currentSessionId = "fallback_1";
    sessionStore.currentSessionInfo = null;
    projectStore.sessionsByProject = new Map();
    projectStore.activeProjects = [];
    projectStore.sidebarProjects = [];
  });

  // ---- activeSessionId ----

  describe("activeSessionId", () => {
    it("reads from route params when available", () => {
      routeParams.sessionId = "route_ses";
      const { activeSessionId } = useSidebarActions();
      expect(activeSessionId.value).toBe("route_ses");
    });

    it("handles array route param by taking the first element", () => {
      routeParams.sessionId = ["arr_1", "arr_2"];
      const { activeSessionId } = useSidebarActions();
      expect(activeSessionId.value).toBe("arr_1");
    });

    it("falls back to session store currentSessionId when route param is missing", () => {
      routeParams.sessionId = undefined;
      sessionStore.currentSessionId = "store_ses";
      const { activeSessionId } = useSidebarActions();
      expect(activeSessionId.value).toBe("store_ses");
    });

    it("returns null when both route param and store are null", () => {
      routeParams.sessionId = undefined;
      sessionStore.currentSessionId = null;
      const { activeSessionId } = useSidebarActions();
      expect(activeSessionId.value).toBeNull();
    });
  });

  // ---- resetDeleteConfirmation ----

  describe("resetDeleteConfirmation", () => {
    it("clears all pending confirmation refs", () => {
      const actions = useSidebarActions();
      actions.pendingDeleteSessionId.value = "ses_1";
      actions.pendingDeleteProjectId.value = "proj_1";
      actions.pendingArchiveProjectSessionId.value = "ps_1";

      actions.resetDeleteConfirmation();

      expect(actions.pendingDeleteSessionId.value).toBeNull();
      expect(actions.pendingDeleteProjectId.value).toBeNull();
      expect(actions.pendingArchiveProjectSessionId.value).toBeNull();
    });
  });

  // ---- switchToSession ----

  describe("switchToSession", () => {
    it("navigates to the workbench with the given sessionId", async () => {
      const actions = useSidebarActions();
      await actions.switchToSession("new_ses");
      expect(mockRouterPush).toHaveBeenCalledWith({
        name: "workbench",
        params: { sessionId: "new_ses" }
      });
    });

    it("does not navigate when sessionId equals the active session", async () => {
      routeParams.sessionId = "same_ses";
      const actions = useSidebarActions();
      await actions.switchToSession("same_ses");
      expect(mockRouterPush).not.toHaveBeenCalled();
    });

    it("resets delete confirmations before switching", async () => {
      const actions = useSidebarActions();
      actions.pendingDeleteSessionId.value = "pending";
      await actions.switchToSession("other_ses");
      expect(actions.pendingDeleteSessionId.value).toBeNull();
    });

    it("handles navigation errors gracefully", async () => {
      mockRouterPush.mockRejectedValueOnce(new Error("nav fail"));
      const actions = useSidebarActions();
      // Should not throw
      await expect(actions.switchToSession("err_ses")).resolves.toBeUndefined();
    });
  });

  // ---- createSession ----

  describe("createSession", () => {
    it("starts an ordinary draft session and navigates to workbench", async () => {
      const actions = useSidebarActions();
      await actions.createSession();
      expect(sessionStore.startOrdinaryDraftSession).toHaveBeenCalledOnce();
      expect(mockRouterPush).toHaveBeenCalledWith({ name: "workbench" });
    });

    it("resets delete confirmations", async () => {
      const actions = useSidebarActions();
      actions.pendingDeleteSessionId.value = "pending";
      await actions.createSession();
      expect(actions.pendingDeleteSessionId.value).toBeNull();
    });

    it("handles errors gracefully", async () => {
      sessionStore.startOrdinaryDraftSession.mockRejectedValueOnce(new Error("fail"));
      const actions = useSidebarActions();
      await expect(actions.createSession()).resolves.toBeUndefined();
    });
  });

  // ---- requestDeleteSession ----

  describe("requestDeleteSession", () => {
    it("sets pendingDeleteSessionId on first call (confirmation step)", async () => {
      const actions = useSidebarActions();
      await actions.requestDeleteSession("ses_del");
      expect(actions.pendingDeleteSessionId.value).toBe("ses_del");
      expect(sessionStore.deleteSession).not.toHaveBeenCalled();
    });

    it("clears pendingDeleteProjectId when requesting session delete", async () => {
      const actions = useSidebarActions();
      actions.pendingDeleteProjectId.value = "proj_1";
      await actions.requestDeleteSession("ses_del");
      expect(actions.pendingDeleteProjectId.value).toBeNull();
    });

    it("deletes the session on second call with same id", async () => {
      const actions = useSidebarActions();
      await actions.requestDeleteSession("ses_del");
      await actions.requestDeleteSession("ses_del");
      expect(sessionStore.deleteSession).toHaveBeenCalledWith("ses_del");
      expect(actions.pendingDeleteSessionId.value).toBeNull();
    });

    it("starts an ordinary draft and clears the route when deleting the active ordinary session", async () => {
      routeParams.sessionId = "ses_active";
      sessionStore.currentSessionId = "ses_active";
      const actions = useSidebarActions();

      await actions.requestDeleteSession("ses_active");
      await actions.requestDeleteSession("ses_active");

      expect(sessionStore.deleteSession).toHaveBeenCalledWith("ses_active");
      expect(sessionStore.startOrdinaryDraftSession).toHaveBeenCalledOnce();
      expect(mockRouterReplace).toHaveBeenCalledWith({ name: "workbench" });
    });

    it("resets to new id when requesting delete of a different session", async () => {
      const actions = useSidebarActions();
      await actions.requestDeleteSession("ses_1");
      await actions.requestDeleteSession("ses_2");
      expect(actions.pendingDeleteSessionId.value).toBe("ses_2");
      expect(sessionStore.deleteSession).not.toHaveBeenCalled();
    });
  });

  // ---- getProjectSessions ----

  describe("getProjectSessions", () => {
    it("returns sessions for a project from the store map", () => {
      const sessions = [makeProjectSession()];
      projectStore.sessionsByProject = new Map([["proj_1", sessions]]);
      const actions = useSidebarActions();
      expect(actions.getProjectSessions("proj_1")).toEqual(sessions);
    });

    it("returns empty array when project has no sessions", () => {
      const actions = useSidebarActions();
      expect(actions.getProjectSessions("unknown")).toEqual([]);
    });
  });

  // ---- switchToProjectSession ----

  describe("switchToProjectSession", () => {
    it("switches and navigates to the project session", async () => {
      const ps = makeProjectSession({ sessionId: "ps_switch" });
      const actions = useSidebarActions();
      await actions.switchToProjectSession(ps);
      expect(sessionStore.switchProjectSession).toHaveBeenCalledWith(ps);
      expect(mockRouterPush).toHaveBeenCalledWith({
        name: "workbench",
        params: { sessionId: "ps_switch" }
      });
    });

    it("resets delete confirmations", async () => {
      const actions = useSidebarActions();
      actions.pendingDeleteSessionId.value = "pending";
      await actions.switchToProjectSession(makeProjectSession());
      expect(actions.pendingDeleteSessionId.value).toBeNull();
    });

    it("handles errors gracefully", async () => {
      sessionStore.switchProjectSession.mockRejectedValueOnce(new Error("fail"));
      const actions = useSidebarActions();
      await expect(actions.switchToProjectSession(makeProjectSession())).resolves.toBeUndefined();
    });
  });

  // ---- createProjectSession ----

  describe("createProjectSession", () => {
    it("starts a project draft session and navigates to workbench", async () => {
      const actions = useSidebarActions();
      await actions.createProjectSession("proj_1");
      expect(sessionStore.startProjectDraftSession).toHaveBeenCalledWith("proj_1");
      expect(mockRouterPush).toHaveBeenCalledWith({ name: "workbench" });
    });

    it("handles errors gracefully", async () => {
      sessionStore.startProjectDraftSession.mockRejectedValueOnce(new Error("fail"));
      const actions = useSidebarActions();
      await expect(actions.createProjectSession("proj_1")).resolves.toBeUndefined();
    });
  });

  // ---- createBlankProject ----

  describe("createBlankProject", () => {
    it("creates a blank project and closes the menu", async () => {
      const actions = useSidebarActions();
      actions.projectCreateMenuOpen.value = true;
      await actions.createBlankProject();
      expect(projectStore.createBlankProject).toHaveBeenCalledOnce();
      expect(actions.projectCreateMenuOpen.value).toBe(false);
    });

    it("handles errors gracefully", async () => {
      projectStore.createBlankProject.mockRejectedValueOnce(new Error("fail"));
      const actions = useSidebarActions();
      await expect(actions.createBlankProject()).resolves.toBeUndefined();
    });
  });

  // ---- importExistingProject ----

  describe("importExistingProject", () => {
    it("imports a project from the selected directory", async () => {
      mockOpen.mockResolvedValueOnce("/tmp/selected");
      const actions = useSidebarActions();
      actions.projectCreateMenuOpen.value = true;
      await actions.importExistingProject();
      expect(projectStore.addExistingProject).toHaveBeenCalledWith("/tmp/selected");
      expect(projectStore.loadProjects).toHaveBeenCalled();
      expect(actions.projectCreateMenuOpen.value).toBe(false);
      expect(actions.importingProject.value).toBe(false);
    });

    it("does nothing when dialog is cancelled (null)", async () => {
      mockOpen.mockResolvedValueOnce(null);
      const actions = useSidebarActions();
      await actions.importExistingProject();
      expect(projectStore.addExistingProject).not.toHaveBeenCalled();
    });

    it("does nothing when dialog returns an array", async () => {
      mockOpen.mockResolvedValueOnce(["/a", "/b"]);
      const actions = useSidebarActions();
      await actions.importExistingProject();
      expect(projectStore.addExistingProject).not.toHaveBeenCalled();
    });

    it("prevents concurrent imports", async () => {
      let resolveOpen: (v: string) => void;
      mockOpen.mockReturnValueOnce(new Promise((r) => (resolveOpen = r)));
      const actions = useSidebarActions();

      const first = actions.importExistingProject();
      // Second call should bail immediately because importingProject is true
      await actions.importExistingProject();
      expect(mockOpen).toHaveBeenCalledTimes(1);

      resolveOpen!("/tmp/path");
      await first;
    });

    it("resets importingProject even when addExistingProject throws", async () => {
      mockOpen.mockResolvedValueOnce("/tmp/err");
      projectStore.addExistingProject.mockRejectedValueOnce(new Error("fail"));
      const actions = useSidebarActions();
      // The error propagates (no catch), but finally still resets importingProject
      await expect(actions.importExistingProject()).rejects.toThrow("fail");
      expect(actions.importingProject.value).toBe(false);
    });
  });

  // ---- requestArchiveProjectSession ----

  describe("requestArchiveProjectSession", () => {
    it("sets pending on first call (confirmation step)", async () => {
      const actions = useSidebarActions();
      await actions.requestArchiveProjectSession("ps_arch");
      expect(actions.pendingArchiveProjectSessionId.value).toBe("ps_arch");
      expect(projectStore.archiveProjectSession).not.toHaveBeenCalled();
    });

    it("archives on second call with same id", async () => {
      const actions = useSidebarActions();
      await actions.requestArchiveProjectSession("ps_arch");
      await actions.requestArchiveProjectSession("ps_arch");
      expect(projectStore.archiveProjectSession).toHaveBeenCalledWith("ps_arch");
      expect(actions.pendingArchiveProjectSessionId.value).toBeNull();
    });

    it("starts an ordinary draft and clears the route when archiving the active project session", async () => {
      routeParams.sessionId = "ps_active";
      sessionStore.currentSessionId = "ps_active";
      const actions = useSidebarActions();

      await actions.requestArchiveProjectSession("ps_active");
      await actions.requestArchiveProjectSession("ps_active");

      expect(projectStore.archiveProjectSession).toHaveBeenCalledWith("ps_active");
      expect(sessionStore.startOrdinaryDraftSession).toHaveBeenCalledOnce();
      expect(mockRouterReplace).toHaveBeenCalledWith({ name: "workbench" });
    });

    it("clears other pending states when requesting archive", async () => {
      const actions = useSidebarActions();
      actions.pendingDeleteSessionId.value = "ses";
      actions.pendingDeleteProjectId.value = "proj";
      await actions.requestArchiveProjectSession("ps_1");
      expect(actions.pendingDeleteSessionId.value).toBeNull();
      expect(actions.pendingDeleteProjectId.value).toBeNull();
    });

    it("handles archive errors gracefully", async () => {
      projectStore.archiveProjectSession.mockRejectedValueOnce(new Error("fail"));
      const actions = useSidebarActions();
      await actions.requestArchiveProjectSession("ps_err");
      await expect(actions.requestArchiveProjectSession("ps_err")).resolves.toBeUndefined();
    });
  });

  // ---- toggleProjectExpanded ----

  describe("toggleProjectExpanded", () => {
    it("expands a collapsed project and loads its sessions", async () => {
      const project = makeProject({ expanded: false });
      const actions = useSidebarActions();
      await actions.toggleProjectExpanded(project);
      expect(projectStore.updateProjectExpanded).toHaveBeenCalledWith("proj_1", true);
      expect(projectStore.loadProjectSessions).toHaveBeenCalledWith("proj_1");
    });

    it("collapses an expanded project without loading sessions", async () => {
      const project = makeProject({ expanded: true });
      const actions = useSidebarActions();
      await actions.toggleProjectExpanded(project);
      expect(projectStore.updateProjectExpanded).toHaveBeenCalledWith("proj_1", false);
      expect(projectStore.loadProjectSessions).not.toHaveBeenCalled();
    });

    it("handles errors gracefully", async () => {
      projectStore.updateProjectExpanded.mockRejectedValueOnce(new Error("fail"));
      const project = makeProject();
      const actions = useSidebarActions();
      await expect(actions.toggleProjectExpanded(project)).resolves.toBeUndefined();
    });
  });

  // ---- requestDeleteProject ----

  describe("requestDeleteProject", () => {
    it("sets pending on first call (confirmation step)", async () => {
      const actions = useSidebarActions();
      await actions.requestDeleteProject("proj_del");
      expect(actions.pendingDeleteProjectId.value).toBe("proj_del");
      expect(projectStore.removeProject).not.toHaveBeenCalled();
    });

    it("clears pendingDeleteSessionId when requesting project delete", async () => {
      const actions = useSidebarActions();
      actions.pendingDeleteSessionId.value = "ses";
      await actions.requestDeleteProject("proj_del");
      expect(actions.pendingDeleteSessionId.value).toBeNull();
    });

    it("deletes the project on second call with same id", async () => {
      const actions = useSidebarActions();
      await actions.requestDeleteProject("proj_del");
      await actions.requestDeleteProject("proj_del");
      expect(projectStore.removeProject).toHaveBeenCalledWith("proj_del");
      expect(actions.pendingDeleteProjectId.value).toBeNull();
    });

    it("starts an ordinary draft and clears the route when deleting the active project", async () => {
      routeParams.sessionId = "ps_active";
      sessionStore.currentSessionId = "ps_active";
      sessionStore.currentSessionInfo = { project_id: "proj_del" };
      const actions = useSidebarActions();

      await actions.requestDeleteProject("proj_del");
      await actions.requestDeleteProject("proj_del");

      expect(projectStore.removeProject).toHaveBeenCalledWith("proj_del");
      expect(sessionStore.startOrdinaryDraftSession).toHaveBeenCalledOnce();
      expect(mockRouterReplace).toHaveBeenCalledWith({ name: "workbench" });
    });
  });

  // ---- loadProjectsForSidebar ----

  describe("loadProjectsForSidebar", () => {
    it("loads projects and sessions for expanded projects", async () => {
      projectStore.sidebarProjects = [
        makeProject({ projectId: "p1", expanded: true }),
        makeProject({ projectId: "p2", expanded: false }),
        makeProject({ projectId: "p3", expanded: true })
      ];
      const actions = useSidebarActions();
      await actions.loadProjectsForSidebar();
      expect(projectStore.loadProjects).toHaveBeenCalledOnce();
      expect(projectStore.loadProjectSessions).toHaveBeenCalledWith("p1");
      expect(projectStore.loadProjectSessions).toHaveBeenCalledWith("p3");
      expect(projectStore.loadProjectSessions).not.toHaveBeenCalledWith("p2");
    });

    it("handles errors gracefully", async () => {
      projectStore.loadProjects.mockRejectedValueOnce(new Error("fail"));
      const actions = useSidebarActions();
      await expect(actions.loadProjectsForSidebar()).resolves.toBeUndefined();
    });
  });

  // ---- initial state ----

  describe("initial state", () => {
    it("starts with all refs in default state", () => {
      const actions = useSidebarActions();
      expect(actions.projectCreateMenuOpen.value).toBe(false);
      expect(actions.pendingDeleteSessionId.value).toBeNull();
      expect(actions.pendingDeleteProjectId.value).toBeNull();
      expect(actions.pendingArchiveProjectSessionId.value).toBeNull();
      expect(actions.importingProject.value).toBe(false);
    });
  });
});

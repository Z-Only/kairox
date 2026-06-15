import { beforeEach, describe, it, expect, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

const mockTraceState = vi.hoisted(() => ({ entries: [] as Array<Record<string, unknown>> }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn()
}));
vi.mock("../composables/useTraceStore", () => ({
  applyTraceEvent: vi.fn(),
  clearTrace: vi.fn(() => {
    mockTraceState.entries = [];
  }),
  traceState: mockTraceState
}));

import { traceState } from "../composables/useTraceStore";
import { useProjectStore } from "@/stores/project";
import { useSessionStore } from "@/stores/session";
import { useWorkspaceUiStore } from "@/stores/workspaceUi";
import {
  installSidebarTestEnv,
  mockInvokeCommandResponses,
  mountSidebar
} from "./SessionsSidebar.test-utils";

installSidebarTestEnv();

beforeEach(() => {
  traceState.entries = [];
});

describe("SessionsSidebar", () => {
  it("keeps project-bound sessions inside expanded projects and out of the regular session list", async () => {
    mockInvokeCommandResponses({
      list_projects: [
        {
          project_id: "project-1",
          display_name: "Demo",
          root_path: "/tmp/demo",
          removed_at: null,
          sort_order: 0,
          expanded: true
        }
      ],
      list_project_sessions: [
        {
          id: "project-session-1",
          title: "Project task",
          profile: "fast",
          project_id: "project-1",
          worktree_path: "/tmp/demo",
          branch: "main",
          visibility: null
        }
      ]
    });
    const { wrapper } = await mountSidebar();
    const session = useSessionStore();

    session.sessions = [{ id: "s1", title: "Regular session", profile: "fast" } as never];
    await flushPromises();

    expect(wrapper.find('[data-test="projects-section"]').text()).toContain("Project task");
    expect(wrapper.find('[data-test="sessions-section"]').text()).toContain("Regular session");
    expect(wrapper.find('[data-test="sessions-section"]').text()).not.toContain("Project task");
  });

  it("opens a project placeholder session without creating backend state", async () => {
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
    const { wrapper, router } = await mountSidebar();
    const projectStore = useProjectStore();
    const sessionStore = useSessionStore();
    const createProjectDraftSession = vi.spyOn(projectStore, "createProjectDraftSession");
    await flushPromises();

    await wrapper.find('[data-test="project-new-session-btn"]').trigger("click");
    await flushPromises();
    await router.isReady();

    expect(createProjectDraftSession).not.toHaveBeenCalled();
    expect(sessionStore.currentSessionId).toBeNull();
    expect(sessionStore.currentSessionInfo?.project_id).toBe("project-1");
    expect(sessionStore.composerDraftKey).toBe("new-session:project:project-1");
    expect(router.currentRoute.value.name).toBe("workbench");
    expect(router.currentRoute.value.params.sessionId).toBeUndefined();
  });

  it("does not expose the old inline branch-name worktree form", async () => {
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

    expect(wrapper.find('[data-test="worktree-branch-input"]').exists()).toBe(false);
    expect(wrapper.find('[data-test^="project-new-worktree-session-btn"]').exists()).toBe(false);
  });

  it("opens the project placeholder from the project row only once", async () => {
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
    const { wrapper, router } = await mountSidebar();
    const sessionStore = useSessionStore();
    await flushPromises();

    await wrapper.find('[data-test="project-new-session-btn"]').trigger("click");
    await flushPromises();
    await router.isReady();

    expect(sessionStore.currentSessionInfo?.project_id).toBe("project-1");
    expect(router.currentRoute.value.name).toBe("workbench");
    expect(router.currentRoute.value.params.sessionId).toBeUndefined();
  });

  it("toggles project expansion through the project store", async () => {
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
    const projectStore = useProjectStore();
    const updateProjectExpanded = vi
      .spyOn(projectStore, "updateProjectExpanded")
      .mockResolvedValue();
    await flushPromises();

    await wrapper.find('[data-test="project-expand-btn"]').trigger("click");
    await flushPromises();

    expect(updateProjectExpanded).toHaveBeenCalledWith("project-1", true);
  });

  it("toggles the project archive section and displays archived sessions", async () => {
    mockInvokeCommandResponses({
      list_archived_sessions: [
        {
          id: "archived-1",
          title: "Archived project task",
          profile: "fast",
          project_id: "project-1",
          worktree_path: "/tmp/demo",
          branch: "main",
          visibility: "archived"
        }
      ]
    });
    const { wrapper } = await mountSidebar();
    const workspaceUi = useWorkspaceUiStore();
    const projectStore = useProjectStore();
    await flushPromises();

    await projectStore.loadArchivedSessions();
    workspaceUi.archiveOpen = true;
    await flushPromises();

    expect(wrapper.find('[data-test="projects-section"]').text()).toContain(
      "Archived project task"
    );

    const archivedSessionTitle = wrapper.find(
      '[data-test="projects-section"] .archived-session-list .session-title'
    );
    expect(archivedSessionTitle.attributes("title")).toBe("Archived project task");
    expect(archivedSessionTitle.classes()).toContain("truncate");
  });

  it("exposes rename and archive actions for project sessions", async () => {
    mockInvokeCommandResponses({
      list_projects: [
        {
          project_id: "project-1",
          display_name: "Demo",
          root_path: "/tmp/demo",
          removed_at: null,
          sort_order: 0,
          expanded: true
        }
      ],
      list_project_sessions: [
        {
          id: "session-1",
          title: "Project task",
          profile: "fast",
          project_id: "project-1",
          worktree_path: "/tmp/demo",
          branch: "main",
          visibility: null
        }
      ]
    });
    const { wrapper } = await mountSidebar();
    await flushPromises();

    expect(wrapper.find('[data-test="project-session-rename-action-session-1"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-test="project-session-archive-action-session-1"]').exists()).toBe(
      true
    );
  });

  it("clears the active project session and route after archiving it", async () => {
    mockInvokeCommandResponses({
      list_projects: [
        {
          project_id: "project-1",
          display_name: "Demo",
          root_path: "/tmp/demo",
          removed_at: null,
          sort_order: 0,
          expanded: true
        }
      ],
      list_project_sessions: [
        {
          id: "project-session-1",
          title: "Project task",
          profile: "fast",
          project_id: "project-1",
          worktree_path: "/tmp/demo",
          branch: "main",
          visibility: null
        }
      ]
    });
    const { wrapper, router } = await mountSidebar();
    const sessionStore = useSessionStore();
    const workspaceUi = useWorkspaceUiStore();
    sessionStore.currentSessionId = "project-session-1";
    sessionStore.projection.messages = [{ role: "user", content: "stale" }];
    workspaceUi.gitReviewContext = { sessionId: "project-session-1", projectId: "project-1" };
    workspaceUi.gitReview = {
      branch: "main",
      changedFiles: ["stale.rs"],
      fileCount: 1,
      additions: 1,
      deletions: 0,
      staged: null,
      unstaged: null,
      untracked: null
    } as never;
    workspaceUi.gitReviewError = "stale error";
    traceState.entries.push({
      id: "ctx-stale",
      kind: "tool",
      status: "completed",
      toolId: "context",
      title: "Context assembled",
      startedAt: Date.now(),
      expanded: false
    });
    await router.push("/workbench/project-session-1");
    await router.isReady();
    await flushPromises();

    await wrapper
      .get('[data-test="project-session-archive-action-project-session-1"]')
      .trigger("click");
    await flushPromises();
    await wrapper
      .get('[data-test="project-session-archive-action-project-session-1"]')
      .trigger("click");
    await flushPromises();
    await router.isReady();

    expect(sessionStore.currentSessionId).toBeNull();
    expect(sessionStore.composerDraftKey).toBe("new-session:ordinary");
    expect(sessionStore.projection.messages).toEqual([]);
    expect(traceState.entries).toEqual([]);
    expect(workspaceUi.gitReviewContext).toBeNull();
    expect(workspaceUi.gitReview).toBeNull();
    expect(workspaceUi.gitReviewError).toBeNull();
    expect(router.currentRoute.value.params.sessionId).toBeUndefined();
  });

  it("clears the active project session and route after deleting its project", async () => {
    mockInvokeCommandResponses({
      list_projects: [
        {
          project_id: "project-1",
          display_name: "Demo",
          root_path: "/tmp/demo",
          removed_at: null,
          sort_order: 0,
          expanded: true
        }
      ],
      list_project_sessions: [
        {
          id: "project-session-1",
          title: "Project task",
          profile: "fast",
          project_id: "project-1",
          worktree_path: "/tmp/demo",
          branch: "main",
          visibility: null
        }
      ]
    });
    const { wrapper, router } = await mountSidebar();
    const sessionStore = useSessionStore();
    const workspaceUi = useWorkspaceUiStore();
    sessionStore.currentSessionId = "project-session-1";
    sessionStore.projection.messages = [{ role: "user", content: "stale" }];
    workspaceUi.gitReviewContext = { sessionId: "project-session-1", projectId: "project-1" };
    workspaceUi.gitReview = {
      branch: "main",
      changedFiles: ["stale.rs"],
      fileCount: 1,
      additions: 1,
      deletions: 0,
      staged: null,
      unstaged: null,
      untracked: null
    } as never;
    workspaceUi.gitReviewError = "stale error";
    traceState.entries.push({
      id: "ctx-stale",
      kind: "tool",
      status: "completed",
      toolId: "context",
      title: "Context assembled",
      startedAt: Date.now(),
      expanded: false
    });
    await router.push("/workbench/project-session-1");
    await router.isReady();
    await flushPromises();

    await wrapper.get('[data-test="project-delete-btn"]').trigger("click");
    await flushPromises();
    await wrapper.get('[data-test="project-delete-confirm"]').trigger("click");
    await flushPromises();
    await router.isReady();

    expect(sessionStore.currentSessionId).toBeNull();
    expect(sessionStore.composerDraftKey).toBe("new-session:ordinary");
    expect(sessionStore.projection.messages).toEqual([]);
    expect(traceState.entries).toEqual([]);
    expect(workspaceUi.gitReviewContext).toBeNull();
    expect(workspaceUi.gitReview).toBeNull();
    expect(workspaceUi.gitReviewError).toBeNull();
    expect(router.currentRoute.value.params.sessionId).toBeUndefined();
  });
});

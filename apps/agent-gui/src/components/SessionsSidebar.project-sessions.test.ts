import { describe, it, expect, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn()
}));
vi.mock("../composables/useTraceStore", () => ({
  applyTraceEvent: vi.fn(),
  clearTrace: vi.fn()
}));

import { useProjectStore } from "@/stores/project";
import { useSessionStore } from "@/stores/session";
import { useWorkspaceUiStore } from "@/stores/workspaceUi";
import {
  installSidebarTestEnv,
  mockInvokeCommandResponses,
  mountSidebar
} from "./SessionsSidebar.test-utils";

installSidebarTestEnv();

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

  it("creates a project draft session and navigates to it", async () => {
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
    const createProjectDraftSession = vi
      .spyOn(projectStore, "createProjectDraftSession")
      .mockResolvedValue({
        sessionId: "draft-1",
        title: "New conversation",
        profile: "fast",
        projectId: "project-1",
        worktreePath: "/tmp/demo",
        branch: null,
        visibility: "draft_hidden"
      });
    await flushPromises();

    await wrapper.find('[data-test="project-new-session-btn"]').trigger("click");
    await flushPromises();
    await router.isReady();

    expect(createProjectDraftSession).toHaveBeenCalledWith("project-1");
    expect(router.currentRoute.value.name).toBe("workbench");
    expect(router.currentRoute.value.params.sessionId).toBe("draft-1");
  });

  it("shows branch input when worktree session button is clicked", async () => {
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

    await wrapper.find('[data-test="project-new-worktree-session-btn-project-1"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="worktree-branch-input"]').exists()).toBe(true);
  });

  it("creates worktree session on confirm with branch name", async () => {
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
    const createProjectWorktreeSession = vi
      .spyOn(projectStore, "createProjectWorktreeSession")
      .mockResolvedValue({
        sessionId: "wt-1",
        title: "New Session (feat-x)",
        profile: "default",
        projectId: "project-1",
        worktreePath: null,
        branch: "feat-x",
        visibility: "visible"
      });
    await flushPromises();

    await wrapper.find('[data-test="project-new-worktree-session-btn-project-1"]').trigger("click");
    await flushPromises();

    await wrapper.find('[data-test="worktree-branch-input"]').setValue("feat-x");
    await wrapper.find('[data-test="worktree-branch-confirm"]').trigger("click");
    await flushPromises();
    await router.isReady();

    expect(createProjectWorktreeSession).toHaveBeenCalledWith("project-1", "feat-x");
    expect(router.currentRoute.value.name).toBe("workbench");
    expect(router.currentRoute.value.params.sessionId).toBe("wt-1");
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
});

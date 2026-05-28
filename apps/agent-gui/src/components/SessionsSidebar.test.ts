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
  mockedInvoke,
  mockInvokeCommandResponses,
  mountSidebar
} from "./SessionsSidebar.test-utils";

installSidebarTestEnv();

describe("SessionsSidebar", () => {
  describe("root empty state", () => {
    it(
      "shows root empty state when both projects and sessions are empty",
      {
        timeout: 15000
      },
      async () => {
        const { wrapper } = await mountSidebar();
        await flushPromises();

        const rootEmpty = wrapper.find('[data-test="sessions-root-empty"]');
        expect(rootEmpty.exists()).toBe(true);
        expect(rootEmpty.classes()).toContain("kx-empty-state");
      }
    );

    it("hides root empty state when sessions exist", async () => {
      const { wrapper } = await mountSidebar();
      const session = useSessionStore();
      session.sessions = [{ id: "s1", title: "Session 1", profile: "fast" } as never];
      await flushPromises();

      expect(wrapper.find('[data-test="sessions-root-empty"]').exists()).toBe(false);
    });

    it("hides root empty state when projects exist", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "Project",
            root_path: "/tmp/p",
            removed_at: null,
            sort_order: 0,
            expanded: false
          }
        ]
      });
      const { wrapper } = await mountSidebar();
      await flushPromises();

      expect(wrapper.find('[data-test="sessions-root-empty"]').exists()).toBe(false);
    });
  });

  describe("active session highlighting", () => {
    it("applies active class to the current session", async () => {
      const { wrapper, router } = await mountSidebar();
      const session = useSessionStore();
      session.sessions = [
        { id: "s1", title: "Session A", profile: "fast" } as never,
        { id: "s2", title: "Session B", profile: "fast" } as never
      ];
      await router.push({ name: "workbench", params: { sessionId: "s1" } });
      await router.isReady();
      await flushPromises();

      const items = wrapper.findAll('[data-test="session-item"]');
      expect(items[0].classes()).toContain("active");
      expect(items[1].classes()).not.toContain("active");
    });

    it("applies active class to the current project session", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "Project",
            root_path: "/tmp/p",
            removed_at: null,
            sort_order: 0,
            expanded: true
          }
        ],
        list_project_sessions: [
          {
            id: "ps1",
            title: "Task A",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/p",
            branch: "main",
            visibility: null
          },
          {
            id: "ps2",
            title: "Task B",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/p",
            branch: "feat",
            visibility: null
          }
        ]
      });
      const { wrapper, router } = await mountSidebar();
      await router.push({ name: "workbench", params: { sessionId: "ps1" } });
      await router.isReady();
      await flushPromises();

      const buttons = wrapper.findAll('[data-test="project-session-btn"]');
      expect(buttons[0].classes()).toContain("active");
      expect(buttons[1].classes()).not.toContain("active");
    });
  });

  describe("session search", () => {
    it("renders the search input with placeholder and aria-label", async () => {
      const { wrapper } = await mountSidebar();

      const input = wrapper.find('[data-test="session-search-input"]');
      expect(input.exists()).toBe(true);
      expect(input.attributes("aria-label")).toBeTruthy();
    });

    it("hides the clear button when search is empty", async () => {
      const { wrapper } = await mountSidebar();

      expect(wrapper.find('[data-test="session-search-clear"]').exists()).toBe(false);
    });

    it("shows the clear button when search has content", async () => {
      const { wrapper } = await mountSidebar();
      await wrapper.get('[data-test="session-search-input"]').setValue("test");
      await flushPromises();

      expect(wrapper.find('[data-test="session-search-clear"]').exists()).toBe(true);
    });

    it("filters sessions case-insensitively", async () => {
      const { wrapper } = await mountSidebar();
      const session = useSessionStore();
      session.sessions = [
        { id: "s1", title: "Release Planning", profile: "fast" } as never,
        { id: "s2", title: "Bug Triage", profile: "slow" } as never
      ];
      await flushPromises();

      await wrapper.get('[data-test="session-search-input"]').setValue("RELEASE");
      await flushPromises();

      expect(wrapper.find('[data-test="sessions-section"]').text()).toContain("Release Planning");
      expect(wrapper.find('[data-test="sessions-section"]').text()).not.toContain("Bug Triage");
    });

    it("filters sessions by profile", async () => {
      const { wrapper } = await mountSidebar();
      const session = useSessionStore();
      session.sessions = [
        { id: "s1", title: "Session A", profile: "claude-sonnet" } as never,
        { id: "s2", title: "Session B", profile: "gpt-4" } as never
      ];
      await flushPromises();

      await wrapper.get('[data-test="session-search-input"]').setValue("sonnet");
      await flushPromises();

      expect(wrapper.find('[data-test="sessions-section"]').text()).toContain("Session A");
      expect(wrapper.find('[data-test="sessions-section"]').text()).not.toContain("Session B");
    });

    it("filters archived sessions by title", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "Project",
            root_path: "/tmp/p",
            removed_at: null,
            sort_order: 0,
            expanded: false
          }
        ],
        list_archived_sessions: [
          {
            id: "as1",
            title: "Old feature work",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/p",
            branch: "main",
            visibility: "archived"
          },
          {
            id: "as2",
            title: "Bug investigation",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/p",
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

      await wrapper.get('[data-test="session-search-input"]').setValue("feature");
      await flushPromises();

      const archiveText = wrapper.find('[data-test="projects-section"]').text();
      expect(archiveText).toContain("Old feature work");
      expect(archiveText).not.toContain("Bug investigation");
    });
  });

  describe("section ordering", () => {
    it("respects reversed section order from workspaceUi store", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "Project",
            root_path: "/tmp/p",
            removed_at: null,
            sort_order: 0,
            expanded: false
          }
        ]
      });
      const { wrapper } = await mountSidebar();
      const session = useSessionStore();
      const workspaceUi = useWorkspaceUiStore();
      session.sessions = [{ id: "s1", title: "Regular", profile: "fast" } as never];
      workspaceUi.sectionOrder = ["sessions", "projects"];
      await flushPromises();

      const sessionsSection = wrapper.find('[data-test="sessions-section"]');
      const projectsSection = wrapper.find('[data-test="projects-section"]');
      expect(sessionsSection.element.compareDocumentPosition(projectsSection.element)).toBe(
        Node.DOCUMENT_POSITION_FOLLOWING
      );
    });
  });

  describe("session rename flow", () => {
    it("completes the rename cycle: start, edit, confirm", async () => {
      const { wrapper } = await mountSidebar();
      const session = useSessionStore();
      session.sessions = [{ id: "s1", title: "Old Title", profile: "fast" } as never];
      await flushPromises();

      await wrapper.find('[data-test="session-rename-btn"]').trigger("click");
      await flushPromises();

      const renameInput = wrapper.find('[data-test="session-rename-input"]');
      expect(renameInput.exists()).toBe(true);

      await renameInput.setValue("New Title");
      await wrapper.find('[data-test="session-rename-confirm"]').trigger("click");
      await flushPromises();

      expect(mockedInvoke).toHaveBeenCalledWith("rename_session", {
        sessionId: "s1",
        title: "New Title"
      });
    });

    it("cancels rename on escape without calling the store", async () => {
      const { wrapper } = await mountSidebar();
      const session = useSessionStore();
      session.sessions = [{ id: "s1", title: "Original", profile: "fast" } as never];
      await flushPromises();

      await wrapper.find('[data-test="session-rename-btn"]').trigger("click");
      await flushPromises();

      await wrapper.find('[data-test="session-rename-input"]').trigger("keydown.escape");
      await flushPromises();

      expect(wrapper.find('[data-test="session-rename-input"]').exists()).toBe(false);
      expect(mockedInvoke).not.toHaveBeenCalledWith(
        "rename_session",
        expect.objectContaining({ sessionId: "s1" })
      );
    });
  });

  describe("project session rename flow", () => {
    it("opens and confirms project session rename", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "Demo",
            root_path: "/tmp/demo",
            removed_at: null,
            sort_order: 0,
            expanded: true
          }
        ],
        list_project_sessions: [
          {
            id: "ps1",
            title: "Old Task",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/demo",
            branch: "main",
            visibility: null
          }
        ]
      });
      const { wrapper } = await mountSidebar();
      await flushPromises();

      await wrapper.find('[data-test="project-session-rename-action-ps1"]').trigger("click");
      await flushPromises();

      const renameInput = wrapper.find('[data-test="project-session-rename-input-ps1"]');
      expect(renameInput.exists()).toBe(true);

      await renameInput.setValue("New Task");
      await wrapper.find('[data-test="project-session-rename-confirm-ps1"]').trigger("click");
      await flushPromises();

      expect(mockedInvoke).toHaveBeenCalledWith("rename_session", {
        sessionId: "ps1",
        title: "New Task"
      });
    });
  });

  describe("project session archive", () => {
    it("requires a second click to confirm project session archive", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "Demo",
            root_path: "/tmp/demo",
            removed_at: null,
            sort_order: 0,
            expanded: true
          }
        ],
        list_project_sessions: [
          {
            id: "ps1",
            title: "Task",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/demo",
            branch: "main",
            visibility: null
          }
        ]
      });
      const { wrapper } = await mountSidebar();
      await flushPromises();

      const archiveBtn = wrapper.find('[data-test="project-session-archive-action-ps1"]');
      await archiveBtn.trigger("click");
      await flushPromises();
      expect(mockedInvoke).not.toHaveBeenCalledWith("delete_session", {
        sessionId: "ps1"
      });

      await wrapper.find('[data-test="project-session-archive-action-ps1"]').trigger("click");
      await flushPromises();
      expect(mockedInvoke).toHaveBeenCalledWith("delete_session", {
        sessionId: "ps1"
      });
    });

    it("shows confirm-action class after first archive click", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "Demo",
            root_path: "/tmp/demo",
            removed_at: null,
            sort_order: 0,
            expanded: true
          }
        ],
        list_project_sessions: [
          {
            id: "ps1",
            title: "Task",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/demo",
            branch: "main",
            visibility: null
          }
        ]
      });
      const { wrapper } = await mountSidebar();
      await flushPromises();

      await wrapper.find('[data-test="project-session-archive-action-ps1"]').trigger("click");
      await flushPromises();

      expect(wrapper.find('[data-test="project-session-archive-action-ps1"]').classes()).toContain(
        "confirm-action"
      );
    });
  });

  describe("project branch display", () => {
    it("shows branch badge for non-main branches on project sessions", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "Demo",
            root_path: "/tmp/demo",
            removed_at: null,
            sort_order: 0,
            expanded: true
          }
        ],
        list_project_sessions: [
          {
            id: "ps1",
            title: "Feature work",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/demo",
            branch: "feat/cool",
            visibility: null
          },
          {
            id: "ps2",
            title: "Main work",
            profile: "fast",
            project_id: "p1",
            worktree_path: "/tmp/demo",
            branch: "main",
            visibility: null
          }
        ]
      });
      const { wrapper } = await mountSidebar();
      await flushPromises();

      const branchBadges = wrapper.findAll(".project-session-branch");
      expect(branchBadges.length).toBe(1);
      expect(branchBadges[0].text()).toBe("feat/cool");
    });
  });

  describe("project display", () => {
    it("shows project path below the display name", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "My Project",
            root_path: "/home/user/projects/cool-app",
            removed_at: null,
            sort_order: 0,
            expanded: false
          }
        ]
      });
      const { wrapper } = await mountSidebar();
      await flushPromises();

      const projectItem = wrapper.find('[data-test="project-item"]');
      expect(projectItem.find(".project-name").text()).toBe("My Project");
      expect(projectItem.find(".project-path").text()).toBe("/home/user/projects/cool-app");
    });

    it("truncates long project names and paths", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "A Very Long Project Name That Should Be Truncated",
            root_path: "/home/user/extremely/deep/nested/path/to/project",
            removed_at: null,
            sort_order: 0,
            expanded: false
          }
        ]
      });
      const { wrapper } = await mountSidebar();
      await flushPromises();

      const projectItem = wrapper.find('[data-test="project-item"]');
      expect(projectItem.find(".project-name").classes()).toContain("truncate");
      expect(projectItem.find(".project-path").classes()).toContain("truncate");
    });
  });

  describe("data-test selectors", () => {
    it("provides sessions-sidebar root selector", async () => {
      const { wrapper } = await mountSidebar();
      expect(wrapper.find('[data-test="sessions-sidebar"]').exists()).toBe(true);
    });

    it("provides sessions-section and projects-section selectors", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "Project",
            root_path: "/tmp/p",
            removed_at: null,
            sort_order: 0,
            expanded: false
          }
        ]
      });
      const { wrapper } = await mountSidebar();
      await flushPromises();

      expect(wrapper.find('[data-test="sessions-section"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="projects-section"]').exists()).toBe(true);
    });

    it("provides scroll region selectors for both sections", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "Project",
            root_path: "/tmp/p",
            removed_at: null,
            sort_order: 0,
            expanded: false
          }
        ]
      });
      const { wrapper } = await mountSidebar();
      await flushPromises();

      expect(wrapper.find('[data-test="sessions-scroll-region"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="projects-scroll-region"]').exists()).toBe(true);
    });
  });

  describe("sidebar loads projects on mount", () => {
    it("calls list_projects when mounted", async () => {
      await mountSidebar();
      await flushPromises();

      expect(mockedInvoke).toHaveBeenCalledWith("list_projects");
    });

    it("loads sessions for expanded projects on mount", async () => {
      mockInvokeCommandResponses({
        list_projects: [
          {
            project_id: "p1",
            display_name: "Expanded",
            root_path: "/tmp/expanded",
            removed_at: null,
            sort_order: 0,
            expanded: true
          },
          {
            project_id: "p2",
            display_name: "Collapsed",
            root_path: "/tmp/collapsed",
            removed_at: null,
            sort_order: 1,
            expanded: false
          }
        ]
      });
      await mountSidebar();
      await flushPromises();

      expect(mockedInvoke).toHaveBeenCalledWith("list_project_sessions", { projectId: "p1" });
    });
  });
});

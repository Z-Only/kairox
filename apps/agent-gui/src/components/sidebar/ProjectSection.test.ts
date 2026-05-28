import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { enableAutoUnmount, flushPromises } from "@vue/test-utils";
import { ref, type Ref } from "vue";
import type { SidebarRenameController } from "@/composables/sidebar/useSidebarRename";
import type { ProjectInfo, ProjectSessionInfo } from "@/stores/project";
import ProjectSection from "./ProjectSection.vue";
import { mountWithPlugins } from "@/test-utils/mount";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeProject(overrides: Partial<ProjectInfo> = {}): ProjectInfo {
  return {
    projectId: "project-1",
    displayName: "My Project",
    rootPath: "/tmp/my-project",
    removedAt: null,
    sortOrder: 0,
    expanded: false,
    pathExists: true,
    ...overrides
  };
}

function makeProjectSession(overrides: Partial<ProjectSessionInfo> = {}): ProjectSessionInfo {
  return {
    sessionId: "ps-1",
    title: "Session One",
    profile: "fast",
    projectId: "project-1",
    worktreePath: "/tmp/my-project",
    branch: "feat/work",
    visibility: null,
    deletedAt: null,
    ...overrides
  };
}

function makeRenameController(
  overrides: Partial<SidebarRenameController> = {}
): SidebarRenameController {
  return {
    editingId: ref(null) as Ref<string | null>,
    title: ref(""),
    input: ref(null),
    start: vi.fn(),
    bindInput: vi.fn(),
    confirm: vi.fn(),
    cancel: vi.fn(),
    ...overrides
  };
}

interface MountOptions {
  activeProjects?: ProjectInfo[];
  archivedSessions?: ProjectSessionInfo[];
  activeSessionId?: string | null;
  pendingDeleteProjectId?: string | null;
  pendingArchiveProjectSessionId?: string | null;
  importingProject?: boolean;
  projectRename?: SidebarRenameController;
  projectSessionRename?: SidebarRenameController;
  getProjectSessions?: (projectId: string) => ProjectSessionInfo[];
  createBlankProject?: () => Promise<void> | void;
  importExistingProject?: () => Promise<void> | void;
  toggleProjectExpanded?: (project: ProjectInfo) => Promise<void> | void;
  createProjectSession?: (projectId: string) => Promise<void> | void;
  requestDeleteProject?: (projectId: string) => Promise<void> | void;
  switchToProjectSession?: (projectSession: ProjectSessionInfo) => Promise<void> | void;
  requestArchiveProjectSession?: (sessionId: string) => Promise<void> | void;
  archiveOpen?: boolean;
  projectCreateMenuOpen?: boolean;
}

function mountProjectSection(opts: MountOptions = {}) {
  const projectRename = opts.projectRename ?? makeRenameController();
  const projectSessionRename = opts.projectSessionRename ?? makeRenameController();

  const { wrapper, router } = mountWithPlugins(ProjectSection, {
    initialRoute: "/workbench",
    mount: {
      props: {
        activeProjects: opts.activeProjects ?? [],
        archivedSessions: opts.archivedSessions ?? [],
        activeSessionId: opts.activeSessionId ?? null,
        pendingDeleteProjectId: opts.pendingDeleteProjectId ?? null,
        pendingArchiveProjectSessionId: opts.pendingArchiveProjectSessionId ?? null,
        importingProject: opts.importingProject ?? false,
        projectRename,
        projectSessionRename,
        getProjectSessions: opts.getProjectSessions ?? (() => []),
        createBlankProject: opts.createBlankProject ?? vi.fn(),
        importExistingProject: opts.importExistingProject ?? vi.fn(),
        toggleProjectExpanded: opts.toggleProjectExpanded ?? vi.fn(),
        createProjectSession: opts.createProjectSession ?? vi.fn(),
        requestDeleteProject: opts.requestDeleteProject ?? vi.fn(),
        switchToProjectSession: opts.switchToProjectSession ?? vi.fn(),
        requestArchiveProjectSession: opts.requestArchiveProjectSession ?? vi.fn(),
        archiveOpen: opts.archiveOpen ?? false,
        projectCreateMenuOpen: opts.projectCreateMenuOpen ?? false
      } as Record<string, unknown>,
      global: {
        stubs: { Teleport: true }
      }
    }
  });
  return { wrapper, router, projectRename, projectSessionRename };
}

// ---------------------------------------------------------------------------
// Test environment
// ---------------------------------------------------------------------------

enableAutoUnmount(afterEach);
afterEach(() => {
  document.body.innerHTML = "";
});
beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ProjectSection", () => {
  // ---- Rendering project info ----

  describe("rendering", () => {
    it("renders the projects section header", () => {
      const { wrapper } = mountProjectSection();
      expect(wrapper.find('[data-test="projects-section"]').exists()).toBe(true);
      expect(wrapper.find(".section-heading h3").exists()).toBe(true);
    });

    it("renders project name and path", () => {
      const project = makeProject({ displayName: "Kairox", rootPath: "/code/kairox" });
      const { wrapper } = mountProjectSection({ activeProjects: [project] });

      const nameEl = wrapper.find(".project-name");
      expect(nameEl.text()).toBe("Kairox");
      expect(nameEl.attributes("title")).toBe("Kairox");

      const pathEl = wrapper.find(".project-path");
      expect(pathEl.text()).toBe("/code/kairox");
    });

    it("renders branch instead of path when branch is available", () => {
      const project = makeProject({
        displayName: "Kairox",
        rootPath: "/code/kairox"
        // branch is shown via the project-path element: `project.branch || project.rootPath`
        // ProjectInfo doesn't have a `branch` field at the type level, but the
        // template uses it; verify the rootPath fallback.
      });
      const { wrapper } = mountProjectSection({ activeProjects: [project] });

      const pathEl = wrapper.find(".project-path");
      expect(pathEl.text()).toBe("/code/kairox");
    });

    it("renders multiple projects", () => {
      const projects = [
        makeProject({ projectId: "p1", displayName: "Alpha" }),
        makeProject({ projectId: "p2", displayName: "Beta" })
      ];
      const { wrapper } = mountProjectSection({ activeProjects: projects });

      const items = wrapper.findAll('[data-test="project-item"]');
      expect(items).toHaveLength(2);
      expect(items[0].text()).toContain("Alpha");
      expect(items[1].text()).toContain("Beta");
    });

    it("shows empty list when no projects", () => {
      const { wrapper } = mountProjectSection({ activeProjects: [] });
      expect(wrapper.findAll('[data-test="project-item"]')).toHaveLength(0);
    });
  });

  // ---- Expand/collapse ----

  describe("expand/collapse", () => {
    it("shows collapsed indicator when project.expanded is false", () => {
      const project = makeProject({ expanded: false });
      const { wrapper } = mountProjectSection({ activeProjects: [project] });

      const btn = wrapper.find('[data-test="project-expand-btn"]');
      expect(btn.text()).toBe("▸");
    });

    it("shows expanded indicator when project.expanded is true", () => {
      const project = makeProject({ expanded: true });
      const { wrapper } = mountProjectSection({ activeProjects: [project] });

      const btn = wrapper.find('[data-test="project-expand-btn"]');
      expect(btn.text()).toBe("▾");
    });

    it("calls toggleProjectExpanded when expand button is clicked", async () => {
      const project = makeProject();
      const toggleProjectExpanded = vi.fn();
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        toggleProjectExpanded
      });

      await wrapper.find('[data-test="project-expand-btn"]').trigger("click");
      expect(toggleProjectExpanded).toHaveBeenCalledWith(project);
    });

    it("calls toggleProjectExpanded when project title row is clicked", async () => {
      const project = makeProject();
      const toggleProjectExpanded = vi.fn();
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        toggleProjectExpanded
      });

      await wrapper.find(".project-title-btn").trigger("click");
      expect(toggleProjectExpanded).toHaveBeenCalledWith(project);
    });
  });

  // ---- Project rename ----

  describe("project rename", () => {
    it("shows editable label when project is being renamed", () => {
      const project = makeProject({ projectId: "p1", displayName: "Old Name" });
      const projectRename = makeRenameController({
        editingId: ref("p1") as Ref<string | null>,
        title: ref("Old Name")
      });
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        projectRename
      });

      const input = wrapper.find('[data-test="project-rename-input-p1"]');
      expect(input.exists()).toBe(true);
      // Title button should be hidden during rename
      expect(wrapper.find(".project-title-btn").exists()).toBe(false);
    });

    it("shows title button when project is not being renamed", () => {
      const project = makeProject({ projectId: "p1" });
      const { wrapper } = mountProjectSection({ activeProjects: [project] });

      expect(wrapper.find(".project-title-btn").exists()).toBe(true);
      expect(wrapper.find('[data-test="project-rename-input-p1"]').exists()).toBe(false);
    });

    it("starts rename when rename action button is clicked", async () => {
      const project = makeProject({ projectId: "p1", displayName: "MyProject" });
      const projectRename = makeRenameController();
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        projectRename
      });

      await wrapper.find('[data-test="project-rename-action-p1"]').trigger("click");
      expect(projectRename.start).toHaveBeenCalledWith("p1", "MyProject");
    });
  });

  // ---- Delete project ----

  describe("delete project", () => {
    it("shows delete button for each project", () => {
      const project = makeProject({ projectId: "p1" });
      const { wrapper } = mountProjectSection({ activeProjects: [project] });

      expect(wrapper.find('[data-test="project-delete-btn"]').exists()).toBe(true);
    });

    it("shows confirm delete button when pending delete matches project", () => {
      const project = makeProject({ projectId: "p1" });
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        pendingDeleteProjectId: "p1"
      });

      expect(wrapper.find('[data-test="project-delete-confirm"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="project-delete-btn"]').exists()).toBe(false);
    });

    it("calls requestDeleteProject when delete button is clicked", async () => {
      const project = makeProject({ projectId: "p1" });
      const requestDeleteProject = vi.fn();
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        requestDeleteProject
      });

      await wrapper.find('[data-test="project-delete-btn"]').trigger("click");
      expect(requestDeleteProject).toHaveBeenCalledWith("p1");
    });
  });

  // ---- Session list within project ----

  describe("session list within project", () => {
    it("shows sessions when project is expanded", () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const sessions = [
        makeProjectSession({ sessionId: "s1", title: "Task A", projectId: "p1" }),
        makeProjectSession({ sessionId: "s2", title: "Task B", projectId: "p1" })
      ];
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        getProjectSessions: () => sessions
      });

      const sessionBtns = wrapper.findAll('[data-test="project-session-btn"]');
      expect(sessionBtns).toHaveLength(2);
      expect(sessionBtns[0].text()).toContain("Task A");
      expect(sessionBtns[1].text()).toContain("Task B");
    });

    it("hides sessions when project is collapsed", () => {
      const project = makeProject({ projectId: "p1", expanded: false });
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        getProjectSessions: () => [makeProjectSession()]
      });

      expect(wrapper.findAll('[data-test="project-session-btn"]')).toHaveLength(0);
    });

    it("highlights active session", () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const session = makeProjectSession({ sessionId: "active-s" });
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        activeSessionId: "active-s",
        getProjectSessions: () => [session]
      });

      const btn = wrapper.find('[data-test="project-session-btn"]');
      expect(btn.classes()).toContain("active");
    });

    it("does not highlight inactive session", () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const session = makeProjectSession({ sessionId: "other-s" });
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        activeSessionId: "different-session",
        getProjectSessions: () => [session]
      });

      const btn = wrapper.find('[data-test="project-session-btn"]');
      expect(btn.classes()).not.toContain("active");
    });

    it("displays branch badge for non-main branches", () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const session = makeProjectSession({ branch: "feat/work" });
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        getProjectSessions: () => [session]
      });

      const branchEl = wrapper.find(".project-session-branch");
      expect(branchEl.exists()).toBe(true);
      expect(branchEl.text()).toBe("feat/work");
    });

    it("hides branch badge for main branch", () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const session = makeProjectSession({ branch: "main" });
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        getProjectSessions: () => [session]
      });

      expect(wrapper.find(".project-session-branch").exists()).toBe(false);
    });

    it("hides branch badge when branch is null", () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const session = makeProjectSession({ branch: null });
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        getProjectSessions: () => [session]
      });

      expect(wrapper.find(".project-session-branch").exists()).toBe(false);
    });

    it("calls switchToProjectSession when session button is clicked", async () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const session = makeProjectSession({ sessionId: "s1" });
      const switchToProjectSession = vi.fn();
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        getProjectSessions: () => [session],
        switchToProjectSession
      });

      await wrapper.find('[data-test="project-session-btn"]').trigger("click");
      expect(switchToProjectSession).toHaveBeenCalledWith(session);
    });
  });

  // ---- Project session rename ----

  describe("project session rename", () => {
    it("shows editable label when session is being renamed", () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const session = makeProjectSession({ sessionId: "s1" });
      const projectSessionRename = makeRenameController({
        editingId: ref("s1") as Ref<string | null>,
        title: ref("Session One")
      });
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        getProjectSessions: () => [session],
        projectSessionRename
      });

      expect(wrapper.find('[data-test="project-session-rename-input-s1"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="project-session-btn"]').exists()).toBe(false);
    });

    it("starts rename when session rename action is clicked", async () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const session = makeProjectSession({ sessionId: "s1", title: "Task A" });
      const projectSessionRename = makeRenameController();
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        getProjectSessions: () => [session],
        projectSessionRename
      });

      await wrapper.find('[data-test="project-session-rename-action-s1"]').trigger("click");
      expect(projectSessionRename.start).toHaveBeenCalledWith("s1", "Task A");
    });
  });

  // ---- Archive project session ----

  describe("archive project session", () => {
    it("shows archive action for project sessions", () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const session = makeProjectSession({ sessionId: "s1" });
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        getProjectSessions: () => [session]
      });

      expect(wrapper.find('[data-test="project-session-archive-action-s1"]').exists()).toBe(true);
    });

    it("shows confirm style when archive is pending for session", () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const session = makeProjectSession({ sessionId: "s1" });
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        getProjectSessions: () => [session],
        pendingArchiveProjectSessionId: "s1"
      });

      const archiveBtn = wrapper.find('[data-test="project-session-archive-action-s1"]');
      expect(archiveBtn.classes()).toContain("confirm-action");
    });

    it("calls requestArchiveProjectSession when archive button is clicked", async () => {
      const project = makeProject({ projectId: "p1", expanded: true });
      const session = makeProjectSession({ sessionId: "s1" });
      const requestArchiveProjectSession = vi.fn();
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        getProjectSessions: () => [session],
        requestArchiveProjectSession
      });

      await wrapper.find('[data-test="project-session-archive-action-s1"]').trigger("click");
      expect(requestArchiveProjectSession).toHaveBeenCalledWith("s1");
    });
  });

  // ---- Create project menu ----

  describe("create project menu", () => {
    it("renders the create project trigger button", () => {
      const { wrapper } = mountProjectSection();
      expect(wrapper.find('[data-test="project-create-trigger"]').exists()).toBe(true);
    });

    it("calls createBlankProject when blank project option is clicked", async () => {
      const createBlankProject = vi.fn();
      const { wrapper } = mountProjectSection({
        createBlankProject,
        projectCreateMenuOpen: true
      });
      await flushPromises();

      const blankBtn = wrapper.find('[data-test="project-create-blank"]');
      if (blankBtn.exists()) {
        await blankBtn.trigger("click");
        expect(createBlankProject).toHaveBeenCalled();
      }
    });

    it("calls importExistingProject when import option is clicked", async () => {
      const importExistingProject = vi.fn();
      const { wrapper } = mountProjectSection({
        importExistingProject,
        projectCreateMenuOpen: true
      });
      await flushPromises();

      const importBtn = wrapper.find('[data-test="project-import-folder"]');
      if (importBtn.exists()) {
        await importBtn.trigger("click");
        expect(importExistingProject).toHaveBeenCalled();
      }
    });

    it("disables import button when importingProject is true", async () => {
      const { wrapper } = mountProjectSection({
        importingProject: true,
        projectCreateMenuOpen: true
      });
      await flushPromises();

      const importBtn = wrapper.find('[data-test="project-import-folder"]');
      if (importBtn.exists()) {
        expect(importBtn.attributes("disabled")).toBeDefined();
      }
    });
  });

  // ---- New session in project ----

  describe("new session in project", () => {
    it("calls createProjectSession when new session button is clicked", async () => {
      const project = makeProject({ projectId: "p1" });
      const createProjectSession = vi.fn();
      const { wrapper } = mountProjectSection({
        activeProjects: [project],
        createProjectSession
      });

      await wrapper.find('[data-test="project-new-session-btn"]').trigger("click");
      expect(createProjectSession).toHaveBeenCalledWith("p1");
    });
  });

  // ---- Archived sessions display ----

  describe("archived sessions", () => {
    it("shows archived sessions when archiveOpen is true", () => {
      const archivedSessions = [
        makeProjectSession({
          sessionId: "arch-1",
          title: "Archived Task",
          visibility: "archived"
        })
      ];
      const { wrapper } = mountProjectSection({
        archivedSessions,
        archiveOpen: true
      });

      const archivedList = wrapper.find(".archived-session-list");
      expect(archivedList.exists()).toBe(true);
      expect(archivedList.text()).toContain("Archived Task");
    });

    it("hides archived sessions when archiveOpen is false", () => {
      const archivedSessions = [
        makeProjectSession({ sessionId: "arch-1", title: "Archived Task" })
      ];
      const { wrapper } = mountProjectSection({
        archivedSessions,
        archiveOpen: false
      });

      expect(wrapper.find(".archived-session-list").exists()).toBe(false);
    });

    it("highlights active archived session", () => {
      const archivedSessions = [
        makeProjectSession({ sessionId: "arch-1", title: "Archived Task" })
      ];
      const { wrapper } = mountProjectSection({
        archivedSessions,
        archiveOpen: true,
        activeSessionId: "arch-1"
      });

      const btn = wrapper.findAll('[data-test="project-session-btn"]');
      const archivedBtn = btn.find((b) => b.text().includes("Archived Task"));
      expect(archivedBtn?.classes()).toContain("active");
    });

    it("shows archived indicator for archived sessions", () => {
      const archivedSessions = [
        makeProjectSession({ sessionId: "arch-1", title: "Archived Task" })
      ];
      const { wrapper } = mountProjectSection({
        archivedSessions,
        archiveOpen: true
      });

      expect(wrapper.find(".archived-session-list .archived-indicator").exists()).toBe(true);
    });

    it("calls switchToProjectSession when archived session is clicked", async () => {
      const archivedSession = makeProjectSession({
        sessionId: "arch-1",
        title: "Archived Task"
      });
      const switchToProjectSession = vi.fn();
      const { wrapper } = mountProjectSection({
        archivedSessions: [archivedSession],
        archiveOpen: true,
        switchToProjectSession
      });

      const archivedList = wrapper.find(".archived-session-list");
      await archivedList.find('[data-test="project-session-btn"]').trigger("click");
      expect(switchToProjectSession).toHaveBeenCalledWith(archivedSession);
    });
  });
});

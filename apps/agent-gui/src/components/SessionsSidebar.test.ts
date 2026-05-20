import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { enableAutoUnmount, flushPromises } from "@vue/test-utils";
import SessionsSidebar from "./SessionsSidebar.vue";
import sessionsSidebarSource from "./SessionsSidebar.vue?raw";
import sessionSectionSource from "./sidebar/SessionSection.vue?raw";
import projectSectionSource from "./sidebar/ProjectSection.vue?raw";
import sidebarActionsSource from "@/composables/sidebar/useSidebarActions.ts?raw";
import { mountWithPlugins } from "@/test-utils/mount";
import { confirmDialogKey } from "@/composables/useConfirm";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

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

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

type InvokeResponses = Record<string, unknown>;

function mockInvokeCommandResponses(responses: InvokeResponses = {}) {
  mockedInvoke.mockImplementation((command) => {
    if (command in responses) {
      return Promise.resolve(responses[command]);
    }

    if (command === "list_projects" || command === "list_project_sessions") {
      return Promise.resolve([]);
    }

    if (command === "list_archived_sessions") {
      return Promise.resolve([]);
    }

    if (command === "switch_session") {
      return Promise.resolve({
        messages: [],
        task_titles: [],
        task_graph: { tasks: [] },
        token_stream: "",
        cancelled: false,
        last_context_usage: null,
        model_limits: null,
        compaction: { type: "Idle" }
      });
    }

    if (command === "get_profile_info" || command === "list_profiles" || command === "get_trace") {
      return Promise.resolve([]);
    }

    return Promise.resolve(null);
  });
}

import { useSessionStore } from "@/stores/session";
import { useProjectStore } from "@/stores/project";
import { useWorkspaceUiStore } from "@/stores/workspaceUi";

// `mountWithPlugins({ initialRoute })` wires Pinia + i18n + the production
// router so the Sidebar's dependencies resolve cleanly.
async function mountSidebar() {
  const hostElement = document.createElement("div");
  document.body.appendChild(hostElement);
  const { wrapper, router } = mountWithPlugins(SessionsSidebar, {
    initialRoute: "/workbench",
    mount: {
      attachTo: hostElement,
      global: {
        provide: {
          [confirmDialogKey as symbol]: { confirm: vi.fn().mockResolvedValue(true) }
        },
        stubs: {
          Teleport: true
        }
      }
    }
  });
  await router.isReady();
  return { wrapper, router };
}

enableAutoUnmount(afterEach);

afterEach(() => {
  document.body.innerHTML = "";
});

beforeEach(() => {
  // `mountWithPlugins` activates a fresh Pinia internally; we just reset
  // mocks here so per-test invoke / store state stays isolated.
  vi.clearAllMocks();
  mockInvokeCommandResponses();
});

describe("SessionsSidebar", () => {
  it("renders session titles", async () => {
    // mountSidebar() activates a fresh Pinia internally; we mutate the
    // session store *after* mount and then re-render so the active Pinia
    // instance the component sees is the same one we modify.
    const { wrapper } = await mountSidebar();
    const session = useSessionStore();
    session.sessions = [
      { id: "s1", title: "Chat about Rust", profile: "fast" } as never,
      { id: "s2", title: "Debug session", profile: "slow" } as never
    ];
    await flushPromises();
    expect(wrapper.text()).toContain("Chat about Rust");
    expect(wrapper.text()).toContain("Debug session");
  });

  it("shows empty hint when no sessions", async () => {
    const { wrapper } = await mountSidebar();
    // The empty-state component renders the description text we passed in.
    const empty = wrapper.find('[data-test="sessions-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("No sessions yet");
    expect(empty.classes()).toContain("kx-empty-state");
    expect(empty.classes()).toContain("kx-empty-state--inline");
  });

  it("removes the redundant sidebar header and keeps the new session action in the sessions section", async () => {
    const { wrapper } = await mountSidebar();

    expect(wrapper.find('[data-test="sessions-sidebar-header"]').exists()).toBe(false);
    expect(
      wrapper.find('[data-test="sessions-section"] [data-test="new-session-btn"]').exists()
    ).toBe(true);
  });

  it("navigates to the workbench route with the session id on click", async () => {
    const { wrapper, router } = await mountSidebar();
    const session = useSessionStore();
    session.sessions = [{ id: "s1", title: "Session 1", profile: "fast" } as never];
    await flushPromises();
    // Use the data-test selector so the assertion does not depend on the
    // ordering or class names of UI component internals.
    await wrapper.find('[data-test="session-item"]').trigger("click");
    // Replace the brittle `setTimeout(0)` flush with `flushPromises()` so
    // the test stays correct under `vi.useFakeTimers()` — see Task 5
    // IMPORTANT #4 follow-up.
    await flushPromises();
    await router.isReady();
    expect(router.currentRoute.value.name).toBe("workbench");
    expect(router.currentRoute.value.params.sessionId).toBe("s1");
  });

  it("creates a default session directly without opening the profile dialog", async () => {
    const { wrapper, router } = await mountSidebar();
    const session = useSessionStore();
    const createSessionSpy = vi.spyOn(session, "createSession").mockResolvedValue({
      id: "ses_default",
      title: "Session using default",
      profile: "default"
    });

    await wrapper.find('[data-test="new-session-btn"]').trigger("click");
    await flushPromises();
    await router.isReady();

    expect(wrapper.find('[data-test="new-session-dialog"]').exists()).toBe(false);
    expect(createSessionSpy).toHaveBeenCalledWith(undefined);
    expect(router.currentRoute.value.params.sessionId).toBe("ses_default");
    expect(mockedInvoke).not.toHaveBeenCalledWith("get_profile_info");
  });

  it("removes obsolete profile dialog and dropdown CSS", () => {
    expectSourceMigration(sessionsSidebarSource, {
      forbidden: [".new-session-dialog", ".profile-dropdown", ".profile-option", ".dialog-actions"]
    });
  });

  it("keeps row actions visually hidden until hover or keyboard focus", () => {
    expectSourceMigration(sessionsSidebarSource, {
      requiredPatterns: [
        /\.row-actions\s*\{[\s\S]*opacity:\s*0/,
        /\.session-item:hover\s+\.row-actions/,
        /\.project-row:hover\s+\.row-actions/,
        /:focus-within\s+\.row-actions/
      ]
    });
  });

  it("uses inline SVG icons rather than emoji action labels", () => {
    const sectionSources = [sessionSectionSource, projectSectionSource].join("\n");
    expect(sectionSources).toContain("<svg");
    expect(sectionSources).not.toContain("✏️");
    expect(sectionSources).not.toContain("🗑️");
  });

  it("P2-S2-sidebar-landmark-name: gives the sessions sidebar a unique accessible name", async () => {
    const { wrapper } = await mountSidebar();

    expect(wrapper.find('[data-test="sessions-sidebar"]').attributes("aria-label")).toBe(
      "Sessions"
    );
  });

  it("P2-S2-session-action-aria-labels: gives icon-only session actions stable accessible names", async () => {
    const { wrapper } = await mountSidebar();
    const session = useSessionStore();
    session.sessions = [{ id: "s1", title: "Session 1", profile: "fast" } as never];
    await flushPromises();

    expect(wrapper.find('[data-test="session-rename-btn"]').attributes("aria-label")).toBe(
      "Rename"
    );
    expect(wrapper.find('[data-test="session-archive-btn"]').attributes("aria-label")).toBe(
      "Archive"
    );
  });

  it("uses Kairox icon buttons and title-backed truncation for regular session rows", async () => {
    const longTitle = "A very long regular session title that should remain discoverable";
    const { wrapper } = await mountSidebar();
    const session = useSessionStore();
    session.sessions = [{ id: "s1", title: longTitle, profile: "fast" } as never];
    await flushPromises();

    const sessionTitle = wrapper.find('[data-test="session-item"] .session-title');
    expect(sessionTitle.attributes("title")).toBe(longTitle);
    expect(sessionTitle.classes()).toContain("truncate");
    expect(wrapper.find('[data-test="session-rename-btn"]').classes()).toContain("kx-icon-button");
    expect(wrapper.find('[data-test="session-archive-btn"]').classes()).toContain("kx-icon-button");

    await wrapper.find('[data-test="session-rename-btn"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="session-rename-confirm"]').classes()).toContain(
      "kx-icon-button"
    );

    await wrapper.find('[data-test="session-rename-input"]').trigger("keydown.escape");
    await flushPromises();
    await wrapper.find('[data-test="session-archive-btn"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="session-archive-btn"]').trigger("click");
    await flushPromises();
    expect(mockedInvoke).toHaveBeenCalledWith("delete_session", { sessionId: "s1" });
  });

  it("P2-S2-new-session-contrast: uses kx-icon-button for the new session action", () => {
    expectSourceMigration(sessionSectionSource, {
      required: ['data-test="new-session-btn"', "<KxIconButton"]
    });
  });

  it("requires a second click on the same session archive button before deleting", async () => {
    const { wrapper } = await mountSidebar();
    const session = useSessionStore();
    session.sessions = [{ id: "s1", title: "Session 1", profile: "fast" } as never];
    await flushPromises();

    await wrapper.find('[data-test="session-archive-btn"]').trigger("click");
    await flushPromises();
    expect(mockedInvoke).not.toHaveBeenCalledWith("delete_session", { sessionId: "s1" });

    await wrapper.find('[data-test="session-archive-btn"]').trigger("click");
    await flushPromises();
    expect(mockedInvoke).toHaveBeenCalledWith("delete_session", { sessionId: "s1" });
  });

  it("waits for session deletion before continuing after confirmation", () => {
    expectSourceMigration(sidebarActionsSource, {
      required: ["await session.deleteSession(sessionId)"],
      forbidden: ["void session.deleteSession"]
    });
  });

  it("imports an existing project from the selected directory", async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(open).mockResolvedValue("/tmp/existing-project");
    mockInvokeCommandResponses({
      add_existing_project: {
        project_id: "project-imported",
        display_name: "existing-project",
        root_path: "/tmp/existing-project",
        removed_at: null,
        sort_order: 0,
        expanded: false
      }
    });

    const { wrapper } = await mountSidebar();
    await wrapper.find('[data-test="project-create-trigger"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="project-import-folder"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("add_existing_project", {
      path: "/tmp/existing-project"
    });
  });

  it("audit anchors: exposes stable session lifecycle pilot selectors", async () => {
    const { wrapper } = await mountSidebar();
    const sessionStore = useSessionStore();
    vi.spyOn(sessionStore, "createSession").mockResolvedValue({
      id: "ses_default",
      title: "Session using default",
      profile: "default"
    });

    await wrapper.find('[data-test="new-session-btn"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="new-session-dialog"]').exists()).toBe(false);

    const session = useSessionStore();
    session.sessions = [{ id: "s1", title: "Session 1", profile: "fast" } as never];
    await flushPromises();

    const renameButton = wrapper.find('[data-test="session-rename-btn"]');
    expect(renameButton.exists()).toBe(true);
    await renameButton.trigger("click");
    await flushPromises();

    expect(wrapper.find(".kx-editable-label").exists()).toBe(true);
    const renameInput = wrapper.find('[data-test="session-rename-input"]');
    const renameConfirm = wrapper.find('[data-test="session-rename-confirm"]');
    expect(renameInput.exists()).toBe(true);
    expect(renameConfirm.exists()).toBe(true);
    expect(renameInput.attributes("data-test")).toBe("session-rename-input");
    expect(renameConfirm.attributes("data-test")).toBe("session-rename-confirm");
  });

  it("renders project navigation above regular sessions by default", async () => {
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
    const session = useSessionStore();

    session.sessions = [{ id: "s1", title: "Regular session", profile: "fast" } as never];
    await flushPromises();

    const projectSection = wrapper.find('[data-test="projects-section"]');
    const sessionsSection = wrapper.find('[data-test="sessions-section"]');

    expect(projectSection.exists()).toBe(true);
    expect(sessionsSection.exists()).toBe(true);
    expect(projectSection.text()).toContain("Demo");
    expect(projectSection.element.compareDocumentPosition(sessionsSection.element)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
  });

  it("keeps project and regular session lists in independent scroll regions", () => {
    expectSourceMigration(projectSectionSource, {
      required: ['data-test="projects-scroll-region"']
    });
    expectSourceMigration(sessionSectionSource, {
      required: ['data-test="sessions-scroll-region"']
    });
    expectSourceMigration(sessionsSidebarSource, {
      requiredPatterns: [
        /\.sessions-sidebar \.sidebar-section\s*\{[\s\S]*max-height:/,
        /\.sessions-sidebar \.sidebar-section-scroll\s*\{[\s\S]*overflow-y:\s*auto/
      ]
    });
  });

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

  it("renders project session navigation items as focusable buttons", async () => {
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
    await flushPromises();

    const projectSessionButton = wrapper.get('[data-test="project-session-btn"]');
    expect(projectSessionButton.element.tagName).toBe("BUTTON");
    expect(projectSessionButton.attributes("type")).toBe("button");
    expect(projectSessionButton.attributes("aria-label")).toBe("Open Project task");

    await projectSessionButton.trigger("click");
    await flushPromises();
    await router.isReady();

    expect(router.currentRoute.value.name).toBe("workbench");
    expect(router.currentRoute.value.params.sessionId).toBe("project-session-1");
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

  it("requires a second click on the same project delete button before removing", async () => {
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

    await wrapper.find('[data-test="project-delete-btn"]').trigger("click");
    await flushPromises();
    expect(mockedInvoke).not.toHaveBeenCalledWith("remove_project", { projectId: "project-1" });
    expect(wrapper.find('[data-test="project-delete-confirm"]').exists()).toBe(true);

    await wrapper.find('[data-test="project-delete-confirm"]').trigger("click");
    await flushPromises();
    expect(mockedInvoke).toHaveBeenCalledWith("remove_project", { projectId: "project-1" });
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

  it("opens the project create menu without creating a project", async () => {
    const { wrapper } = await mountSidebar();
    const projectStore = useProjectStore();
    const createBlankProject = vi.spyOn(projectStore, "createBlankProject");
    await flushPromises();

    expect(wrapper.find('[data-test="project-create-menu"]').exists()).toBe(false);

    await wrapper.find('[data-test="project-create-trigger"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="project-create-blank"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="project-import-folder"]').exists()).toBe(true);
    expect(createBlankProject).not.toHaveBeenCalled();
  });

  it("opens an inline project rename input from the project action", async () => {
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

    await wrapper.find('[data-test="project-rename-action-project-1"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="project-rename-input-project-1"]').exists()).toBe(true);
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

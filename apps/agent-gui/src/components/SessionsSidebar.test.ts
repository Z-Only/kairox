import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import SessionsSidebar from "./SessionsSidebar.vue";
import sessionsSidebarSource from "./SessionsSidebar.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";
import { confirmDialogKey } from "@/composables/useConfirm";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
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
  const { wrapper, router } = mountWithPlugins(SessionsSidebar, {
    initialRoute: "/workbench",
    mount: {
      global: {
        provide: {
          [confirmDialogKey as symbol]: { confirm: vi.fn().mockResolvedValue(true) }
        }
      }
    }
  });
  await router.isReady();
  return { wrapper, router };
}

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
    expect(wrapper.text()).toContain("No sessions yet");
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

  it("opens new session dialog on + New click", async () => {
    mockInvokeCommandResponses({
      get_profile_info: [
        {
          alias: "fast",
          provider: "openai",
          model_id: "gpt-4o",
          local: false,
          has_api_key: true
        }
      ]
    });
    const { wrapper } = await mountSidebar();
    await wrapper.find('[data-test="new-session-btn"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("New Session");
  });

  it("P1-S2-session-actions-visible-without-hover: keeps rename and delete actions visible for audit and keyboard discovery", () => {
    expect(sessionsSidebarSource).not.toMatch(/\.session-actions\s*\{[^}]*display:\s*none/);
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
    expect(wrapper.find('[data-test="session-delete-btn"]').attributes("aria-label")).toBe(
      "Delete"
    );
  });

  it("P2-S2-new-session-contrast: uses dedicated high-contrast colors for the new session button", () => {
    expect(sessionsSidebarSource).toContain("--sessions-new-button-bg");
    expect(sessionsSidebarSource).not.toMatch(
      /\.new-session-btn\s*\{[^}]*background:\s*var\(--app-primary-color\)/
    );
  });

  it("waits for session deletion before continuing after confirmation", () => {
    expect(sessionsSidebarSource).not.toContain("void session.deleteSession");
    expect(sessionsSidebarSource).toContain("await session.deleteSession(sessionId)");
  });

  it("audit anchors: exposes stable session lifecycle pilot selectors", async () => {
    mockInvokeCommandResponses({
      get_profile_info: [
        {
          alias: "fast",
          provider: "openai",
          model_id: "gpt-4o",
          local: false,
          has_api_key: true
        }
      ]
    });
    const { wrapper } = await mountSidebar();

    await wrapper.find('[data-test="new-session-btn"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="new-session-dialog"]').exists()).toBe(true);

    const session = useSessionStore();
    session.sessions = [{ id: "s1", title: "Session 1", profile: "fast" } as never];
    await flushPromises();

    const renameButton = wrapper.find('[data-test="session-rename-btn"]');
    expect(renameButton.exists()).toBe(true);
    await renameButton.trigger("click");
    await flushPromises();

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
    await flushPromises();

    expect(workspaceUi.archiveOpen).toBe(false);
    await wrapper.find('[data-test="project-archive-toggle"]').trigger("click");
    await flushPromises();

    expect(workspaceUi.archiveOpen).toBe(true);
    expect(wrapper.find('[data-test="projects-section"]').text()).toContain(
      "Archived project task"
    );
  });
});

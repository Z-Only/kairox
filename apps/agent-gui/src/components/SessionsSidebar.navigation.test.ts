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

import { useSessionStore } from "@/stores/session";
import {
  installSidebarTestEnv,
  mockInvokeCommandResponses,
  mountSidebar
} from "./SessionsSidebar.test-utils";

installSidebarTestEnv();

describe("SessionsSidebar", () => {
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

  it("renders project session navigation items as focusable buttons", async () => {
    mockInvokeCommandResponses({
      list_projects: [
        {
          project_id: "project-1",
          display_name: "Demo",
          root_path: "/tmp/demo",
          removed_at: null,
          sort_order: 0,
          expanded: true,
          path_exists: true
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
});

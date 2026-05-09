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

import { useSessionStore } from "@/stores/session";

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
    mockedInvoke.mockResolvedValueOnce([
      {
        alias: "fast",
        provider: "openai",
        model_id: "gpt-4o",
        local: false,
        has_api_key: true
      }
    ]);
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
    mockedInvoke.mockResolvedValueOnce([
      {
        alias: "fast",
        provider: "openai",
        model_id: "gpt-4o",
        local: false,
        has_api_key: true
      }
    ]);
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
});

import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import SessionsSidebar from "./SessionsSidebar.vue";
import { mountWithPlugins } from "@/test-utils/mount";

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

// `mountWithPlugins({ withNaiveProviders: true, initialRoute })` (added in
// Task 7a) wires Pinia + i18n + the production router and wraps the
// component in the same NaiveUI provider stack as `AppLayout.vue` so
// `useDialog()` and the migrated NaiveUI components resolve cleanly. The
// Sidebar exercises that helper as one of its intended consumers.
async function mountSidebar() {
  const { wrapper, router } = mountWithPlugins(SessionsSidebar, {
    withNaiveProviders: true,
    initialRoute: "/workbench"
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
    // NEmpty renders the description text we passed in.
    expect(wrapper.text()).toContain("No sessions yet");
  });

  it("navigates to the workbench route with the session id on click", async () => {
    const { wrapper, router } = await mountSidebar();
    const session = useSessionStore();
    session.sessions = [
      { id: "s1", title: "Session 1", profile: "fast" } as never
    ];
    await flushPromises();
    // Use the data-test selector so the assertion does not depend on the
    // ordering or class names of NaiveUI internals.
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
});

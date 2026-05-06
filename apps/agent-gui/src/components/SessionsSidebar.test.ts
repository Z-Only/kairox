import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import { createRouter, createMemoryHistory } from "vue-router";
import SessionsSidebar from "./SessionsSidebar.vue";

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

// A stub router that mirrors the production route names but uses inert
// component shims so we don't pull WorkbenchView (and its dependencies)
// into store-only sidebar tests. Sidebar tests only need the router
// plugin present so `useRoute()` / `useRouter()` resolve.
function makeStubRouter() {
  const stub = { template: "<div />" };
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", redirect: { name: "workbench" } },
      {
        path: "/workbench/:sessionId?",
        name: "workbench",
        component: stub,
        props: true
      },
      { path: "/marketplace", name: "marketplace", component: stub },
      { path: "/settings", name: "settings", component: stub }
    ]
  });
}

async function mountSidebar() {
  const router = makeStubRouter();
  await router.push({ name: "workbench" });
  await router.isReady();
  const wrapper = mount(SessionsSidebar, {
    global: { plugins: [router] }
  });
  return { wrapper, router };
}

beforeEach(() => {
  setActivePinia(createPinia());
  const session = useSessionStore();
  session.sessions = [];
  session.currentSessionId = null;
  session.currentProfile = "fast";
  session.resetProjection();
  vi.clearAllMocks();
});

describe("SessionsSidebar", () => {
  it("renders session titles", async () => {
    const session = useSessionStore();
    session.sessions = [
      { id: "s1", title: "Chat about Rust", profile: "fast" } as never,
      { id: "s2", title: "Debug session", profile: "slow" } as never
    ];
    const { wrapper } = await mountSidebar();
    expect(wrapper.text()).toContain("Chat about Rust");
    expect(wrapper.text()).toContain("Debug session");
  });

  it("shows empty hint when no sessions", async () => {
    const { wrapper } = await mountSidebar();
    expect(wrapper.text()).toContain("No sessions yet");
  });

  it("navigates to the workbench route with the session id on click", async () => {
    const session = useSessionStore();
    session.sessions = [
      { id: "s1", title: "Session 1", profile: "fast" } as never
    ];
    const { wrapper, router } = await mountSidebar();
    await wrapper.find(".session-item").trigger("click");
    // Flush the async click handler so router.push resolves.
    await new Promise((r) => setTimeout(r, 0));
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
    await wrapper.find(".new-session-btn").trigger("click");
    expect(wrapper.text()).toContain("New Session");
  });
});

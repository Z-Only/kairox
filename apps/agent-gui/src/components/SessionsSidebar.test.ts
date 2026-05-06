import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount, flushPromises } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { createRouter, createMemoryHistory, type Router } from "vue-router";
import { createI18n } from "vue-i18n";
import {
  NConfigProvider,
  NMessageProvider,
  NDialogProvider,
  NLoadingBarProvider,
  NNotificationProvider
} from "naive-ui";
import SessionsSidebar from "./SessionsSidebar.vue";
import en from "@/locales/en.json";

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
function makeStubRouter(): Router {
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

function makeI18n() {
  return createI18n({ legacy: false, locale: "en", messages: { en } });
}

// Wrap the sidebar in NaiveUI's provider stack so `useDialog()` resolves and
// the migrated `<NScrollbar>` / `<NButton>` / `<NEmpty>` components have
// access to theme + service contexts (mirrors `AppLayout.vue`).
const SidebarHarness = defineComponent({
  name: "SidebarHarness",
  components: { SessionsSidebar },
  setup() {
    return () =>
      h(NConfigProvider, null, {
        default: () =>
          h(NLoadingBarProvider, null, {
            default: () =>
              h(NMessageProvider, null, {
                default: () =>
                  h(NDialogProvider, null, {
                    default: () =>
                      h(NNotificationProvider, null, {
                        default: () => h(SessionsSidebar)
                      })
                  })
              })
          })
      });
  }
});

async function mountSidebar() {
  const router = makeStubRouter();
  await router.push({ name: "workbench" });
  await router.isReady();
  const wrapper = mount(SidebarHarness, {
    global: { plugins: [router, makeI18n()] }
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
    // NEmpty renders the description text we passed in.
    expect(wrapper.text()).toContain("No sessions yet");
  });

  it("navigates to the workbench route with the session id on click", async () => {
    const session = useSessionStore();
    session.sessions = [
      { id: "s1", title: "Session 1", profile: "fast" } as never
    ];
    const { wrapper, router } = await mountSidebar();
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

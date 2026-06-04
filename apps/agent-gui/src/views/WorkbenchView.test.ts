import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createTestingPinia } from "@pinia/testing";
// `createI18n` / `createRouter` / `createMemoryHistory` are not part of
// `unplugin-auto-import`'s default `vue-i18n` / `vue-router` presets
// (the presets only expose the runtime hooks `useI18n`/`useRoute`/
// `useRouter`). Test setup that instantiates a fresh i18n / router
// per spec must keep these imports explicit.
import { createI18n } from "vue-i18n";
import { createRouter, createMemoryHistory } from "vue-router";
import { routes } from "@/router/routes";
import en from "@/locales/en.json";
import { useUiStore } from "@/stores/ui";
import { useSessionStore } from "@/stores/session";
import { useProjectStore } from "@/stores/project";
import WorkbenchView from "./WorkbenchView.vue";
import workbenchSource from "./WorkbenchView.vue?raw";
import chatComposerSource from "@/components/ChatComposer.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

// Stub Tauri plumbing pulled in transitively by ChatPanel / SessionsSidebar
// children. The Pre-work A regression test only cares about WorkbenchView's
// URL <-> store sync logic; the children themselves are stubbed via
// `global.stubs` at mount time so we don't need to mock their internal
// composables here.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

function makeRouter() {
  return createRouter({ history: createMemoryHistory(), routes });
}

function makeI18n() {
  return createI18n({ legacy: false, locale: "en", messages: { en } });
}

beforeEach(() => {
  vi.clearAllMocks();
  window.localStorage.clear();
});

describe("WorkbenchView (Pre-work A regression)", () => {
  it("redirects URL to /workbench when switchSession rejects, and the reverse watcher does not rewrite the bad id back", async () => {
    const router = makeRouter();
    const pinia = createTestingPinia({ createSpy: vi.fn });

    // Stub session store BEFORE pushing the route so the onMounted hook sees
    // the rejection. `useSessionStore(pinia)` returns the same instance the
    // component will resolve via injection.
    const session = useSessionStore(pinia);
    (session.switchSession as unknown as ReturnType<typeof vi.fn>) = vi
      .fn()
      .mockRejectedValue(new Error("Session not found: badId"));

    const ui = useUiStore(pinia);

    await router.push("/workbench/badId");
    await router.isReady();

    mount(WorkbenchView, {
      global: {
        plugins: [router, pinia, makeI18n()],
        stubs: {
          SessionsSidebar: true,
          ChatPanel: true,
          TraceTimeline: true
        }
      }
    });

    // Flush the async onMounted -> switchSession rejection -> router.replace
    // chain.
    await flushPromises();
    await nextTick();

    const params1 = router.currentRoute.value.params;
    const id1 = params1.sessionId;
    expect(id1 === undefined || id1 === "" || !("sessionId" in params1)).toBe(true);

    // Second flush: if the reverse watcher were going to rewrite the bad id
    // back into the URL, it would do so on the next microtask after the
    // store mutation. The syncing guard in WorkbenchView must prevent this.
    await flushPromises();
    await nextTick();

    const params2 = router.currentRoute.value.params;
    const id2 = params2.sessionId;
    expect(id2 === undefined || id2 === "" || !("sessionId" in params2)).toBe(true);

    expect(ui.pushNotification).toHaveBeenCalledTimes(1);
    const call = (ui.pushNotification as unknown as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(call[0]).toBe("error");
    expect(String(call[1])).toContain("badId");
  });

  it("restores project sessions addressed directly by the workbench route", async () => {
    const router = makeRouter();
    const pinia = createTestingPinia({ createSpy: vi.fn });
    const session = useSessionStore(pinia);
    const projectStore = useProjectStore(pinia);
    const ui = useUiStore(pinia);

    (session.switchSession as unknown as ReturnType<typeof vi.fn>) = vi
      .fn()
      .mockRejectedValueOnce(new Error("Project session not loaded yet"))
      .mockResolvedValueOnce(undefined);
    (projectStore.restoreProjectSession as unknown as ReturnType<typeof vi.fn>) = vi
      .fn()
      .mockResolvedValue({
        projectId: "project-1",
        displayName: "Project",
        rootPath: "/tmp/project",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      });

    await router.push("/workbench/project-session-1");
    await router.isReady();

    mount(WorkbenchView, {
      global: {
        plugins: [router, pinia, makeI18n()],
        stubs: {
          SessionsSidebar: true,
          ChatPanel: true,
          TraceTimeline: true
        }
      }
    });

    await flushPromises();
    await nextTick();

    expect(session.switchSession).toHaveBeenCalledTimes(2);
    expect(session.switchSession).toHaveBeenNthCalledWith(1, "project-session-1");
    expect(projectStore.restoreProjectSession).toHaveBeenCalledWith("project-session-1");
    expect(session.switchSession).toHaveBeenNthCalledWith(2, "project-session-1");
    expect(ui.pushNotification).not.toHaveBeenCalled();
    expect(router.currentRoute.value.params.sessionId).toBe("project-session-1");
  });

  it("audit anchors: exposes stable workbench pilot selector", async () => {
    const router = makeRouter();
    const pinia = createTestingPinia({ createSpy: vi.fn });

    await router.push("/workbench");
    await router.isReady();

    const wrapper = mount(WorkbenchView, {
      global: {
        plugins: [router, pinia, makeI18n()],
        stubs: {
          SessionsSidebar: true,
          ChatPanel: true,
          TraceTimeline: true
        }
      }
    });

    expect(wrapper.find('[data-test="view-workbench"]').exists()).toBe(true);
  });

  it("audit accessibility: provides a page-level heading for the workbench", async () => {
    const router = makeRouter();
    const pinia = createTestingPinia({ createSpy: vi.fn });

    await router.push("/workbench");
    await router.isReady();

    const wrapper = mount(WorkbenchView, {
      global: {
        plugins: [router, pinia, makeI18n()],
        stubs: {
          SessionsSidebar: true,
          ChatPanel: true,
          TraceTimeline: true
        }
      }
    });

    const heading = wrapper.find('h1[data-test="workbench-heading"]');
    expect(heading.exists()).toBe(true);
    expect(heading.text()).toBe("Workbench");
  });

  it("toggles the left and right sidebar collapse state", async () => {
    const router = makeRouter();
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });

    await router.push("/workbench");
    await router.isReady();

    const wrapper = mount(WorkbenchView, {
      global: {
        plugins: [router, pinia, makeI18n()],
        stubs: {
          SessionsSidebar: true,
          ChatPanel: true,
          TraceTimeline: true
        }
      }
    });

    const ui = useUiStore(pinia);
    await wrapper.get('[data-test="left-sidebar-toggle"]').trigger("click");
    await wrapper.get('[data-test="right-sidebar-toggle"]').trigger("click");

    expect(ui.leftSidebarCollapsed).toBe(true);
    expect(ui.rightSidebarCollapsed).toBe(true);
    expect(wrapper.get('[data-test="view-workbench"]').classes()).toContain(
      "workbench--left-collapsed"
    );
    expect(wrapper.get('[data-test="view-workbench"]').classes()).toContain(
      "workbench--right-collapsed"
    );
  });

  it("resizes the left sidebar and persists the new width", async () => {
    const router = makeRouter();
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });

    await router.push("/workbench");
    await router.isReady();

    const wrapper = mount(WorkbenchView, {
      attachTo: document.body,
      global: {
        plugins: [router, pinia, makeI18n()],
        stubs: {
          SessionsSidebar: true,
          ChatPanel: true,
          TraceTimeline: true
        }
      }
    });

    const ui = useUiStore(pinia);

    wrapper
      .get('[data-test="left-sidebar-resizer"]')
      .element.dispatchEvent(new PointerEvent("pointerdown", { clientX: 220, bubbles: true }));
    window.dispatchEvent(new PointerEvent("pointermove", { clientX: 260 }));
    window.dispatchEvent(new PointerEvent("pointerup"));
    await nextTick();

    expect(ui.leftSidebarWidth).toBe(260);
    expect(window.localStorage.getItem("kairox.left-sidebar-width")).toBe("260");

    wrapper.unmount();
  });

  it("does not mount the context meter pill in the workbench shell", async () => {
    const router = makeRouter();
    const pinia = createTestingPinia({ createSpy: vi.fn });

    await router.push("/workbench");
    await router.isReady();

    const wrapper = mount(WorkbenchView, {
      global: {
        plugins: [router, pinia, makeI18n()],
        stubs: {
          SessionsSidebar: true,
          ChatPanel: true,
          TraceTimeline: true
        }
      }
    });

    expect(wrapper.find('[data-test="workbench-context-meter-pill"]').exists()).toBe(false);
  });

  it("source: ChatComposer owns the ContextMeterPill mount and WorkbenchView does not", () => {
    expectSourceMigration(workbenchSource, {
      forbidden: ["ContextMeterPill", "workbench-context-meter-pill"]
    });
    expectSourceMigration(chatComposerSource, {
      required: ["ContextMeterPill", "composer-context-meter"],
      forbidden: ["<ContextMeter ", "context-meter-ring", "context-meter-bar"]
    });
  });

  it("resizes the right sidebar using inverse drag direction", async () => {
    const router = makeRouter();
    const pinia = createTestingPinia({ createSpy: vi.fn, stubActions: false });

    await router.push("/workbench");
    await router.isReady();

    const wrapper = mount(WorkbenchView, {
      attachTo: document.body,
      global: {
        plugins: [router, pinia, makeI18n()],
        stubs: {
          SessionsSidebar: true,
          ChatPanel: true,
          TraceTimeline: true
        }
      }
    });

    const ui = useUiStore(pinia);

    wrapper
      .get('[data-test="right-sidebar-resizer"]')
      .element.dispatchEvent(new PointerEvent("pointerdown", { clientX: 900, bubbles: true }));
    window.dispatchEvent(new PointerEvent("pointermove", { clientX: 850 }));
    window.dispatchEvent(new PointerEvent("pointerup"));
    await nextTick();

    expect(ui.rightSidebarWidth).toBe(330);
    expect(window.localStorage.getItem("kairox.right-sidebar-width")).toBe("330");

    wrapper.unmount();
  });
});

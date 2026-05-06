import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";
import { createRouter, createMemoryHistory } from "vue-router";
import { createTestingPinia } from "@pinia/testing";
import { createI18n } from "vue-i18n";
import { routes } from "@/router/routes";
import en from "@/locales/en.json";
import { useUiStore } from "@/stores/ui";
import { useSessionStore } from "@/stores/session";
import WorkbenchView from "./WorkbenchView.vue";

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
          TraceTimeline: true,
          PermissionCenter: true
        }
      }
    });

    // Flush the async onMounted -> switchSession rejection -> router.replace
    // chain.
    await flushPromises();
    await nextTick();

    const params1 = router.currentRoute.value.params;
    const id1 = params1.sessionId;
    expect(id1 === undefined || id1 === "" || !("sessionId" in params1)).toBe(
      true
    );

    // Second flush: if the reverse watcher were going to rewrite the bad id
    // back into the URL, it would do so on the next microtask after the
    // store mutation. The syncing guard in WorkbenchView must prevent this.
    await flushPromises();
    await nextTick();

    const params2 = router.currentRoute.value.params;
    const id2 = params2.sessionId;
    expect(id2 === undefined || id2 === "" || !("sessionId" in params2)).toBe(
      true
    );

    expect(ui.pushNotification).toHaveBeenCalledTimes(1);
    const call = (ui.pushNotification as unknown as ReturnType<typeof vi.fn>)
      .mock.calls[0];
    expect(call[0]).toBe("error");
    expect(String(call[1])).toContain("badId");
  });
});

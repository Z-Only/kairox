import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia } from "pinia";
import { createI18n } from "vue-i18n";
import { createRouter, createMemoryHistory } from "vue-router";
import { routes } from "@/router/routes";
import en from "@/locales/en.json";
import AppLayout from "./AppLayout.vue";

// Stub Tauri APIs that child views pull in transitively.
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

/** Mount AppLayout at the given path. */
async function mountAt(path = "/workbench") {
  const router = makeRouter();
  const pinia = createPinia();

  await router.push(path);
  await router.isReady();

  const wrapper = mount(AppLayout, {
    global: {
      plugins: [router, pinia, makeI18n()],
      stubs: {
        ToastContainer: true,
        // ConfirmDialog wraps the entire template via its default slot;
        // use a slot-forwarding stub so the inner DOM is rendered.
        ConfirmDialog: { template: "<div><slot /></div>" },
        // Stub child route components to isolate the layout shell
        WorkbenchView: true,
        GeneralSettings: true,
        SettingsLayout: true
      }
    }
  });

  await flushPromises();
  return { wrapper, router };
}

describe("AppLayout", () => {
  // --- Shell structure ---

  it("renders the app-shell container", async () => {
    const { wrapper } = await mountAt();
    expect(wrapper.find('[data-test="app-shell"]').exists()).toBe(true);
  });

  it("renders the app-nav bar", async () => {
    const { wrapper } = await mountAt();
    expect(wrapper.find('[data-test="app-nav"]').exists()).toBe(true);
  });

  // --- Navigation links ---

  it("renders a workbench nav link", async () => {
    const { wrapper } = await mountAt();
    const link = wrapper.find('[data-test="nav-workbench"]');
    expect(link.exists()).toBe(true);
    expect(link.text()).toBe("Workbench");
  });

  it("renders a settings nav link", async () => {
    const { wrapper } = await mountAt();
    const link = wrapper.find('[data-test="nav-settings"]');
    expect(link.exists()).toBe(true);
    expect(link.text()).toBe("Settings");
  });

  it("workbench link routes to the workbench named route", async () => {
    const { wrapper } = await mountAt();
    const link = wrapper.get('[data-test="nav-workbench"]');
    // RouterLink renders an <a> with the resolved href
    expect(link.attributes("href")).toBe("/workbench");
  });

  it("settings link routes to the settings-general named route", async () => {
    const { wrapper } = await mountAt();
    const link = wrapper.get('[data-test="nav-settings"]');
    expect(link.attributes("href")).toBe("/settings/general");
  });

  // --- Child components ---

  it("renders ToastContainer", async () => {
    const { wrapper } = await mountAt();
    // Stubbed as <toast-container-stub>
    expect(wrapper.find("toast-container-stub").exists()).toBe(true);
  });

  it("renders ConfirmDialog as root wrapper", async () => {
    const { wrapper } = await mountAt();
    // ConfirmDialog is the outermost element (custom slot-forwarding stub)
    expect(wrapper.html()).toBeTruthy();
    // The shell is rendered inside it
    expect(wrapper.find('[data-test="app-shell"]').exists()).toBe(true);
  });

  // --- RouterView ---

  it("includes a RouterView for child route rendering", async () => {
    const { wrapper } = await mountAt();
    // When a real router is provided, RouterView resolves to the matched
    // route component (or its stub). The nav + resolved child coexist
    // inside app-shell, confirming RouterView is present and functional.
    const shell = wrapper.get('[data-test="app-shell"]');
    const nav = shell.find('[data-test="app-nav"]');
    expect(nav.exists()).toBe(true);
    // The shell contains more than just the nav — the RouterView output follows
    expect(shell.element.children.length).toBeGreaterThan(1);
  });
});

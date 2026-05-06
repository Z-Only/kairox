import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue([])
}));

import { invoke } from "@tauri-apps/api/core";
import { useCatalogStore } from "@/stores/catalog";
import Marketplace from "../../views/MarketplaceView.vue";
import CatalogCard from "./CatalogCard.vue";
import RuntimeMissingHint from "./RuntimeMissingHint.vue";
import InstalledList from "./InstalledList.vue";

// MarketplaceView now calls `useI18n()` (Task 5 NIT #9 follow-up done in
// Task 7c) so mounting it through plain `mount()` would fail with
// "Need to install with `app.use` function". `mountWithPlugins` wires the
// shared i18n + Pinia + router stack the same way every other view test
// does. NaiveUI providers are not required because MarketplaceView itself
// does not call any service hook (`useMessage`, `useDialog`, etc.).
function mountMarketplace() {
  return mountWithPlugins(Marketplace, { initialRoute: "/marketplace" })
    .wrapper;
}

const fixtureEntry = (over: Partial<Record<string, unknown>> = {}) => ({
  id: "filesystem",
  source: "builtin",
  display_name: "Filesystem",
  summary: "Read & write files",
  description: "desc",
  categories: ["filesystem"],
  tags: ["files"],
  author: null,
  homepage: null,
  version: null,
  trust: "verified",
  icon: "📁",
  install_spec_json: "{}",
  requirements_json: "[]",
  default_env_json: "[]",
  ...over
});

describe("Marketplace.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("renders Browse and Installed tabs", async () => {
    const wrapper = mountMarketplace();
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).toContain("Browse");
    expect(wrapper.text()).toContain("Installed");
  });

  it("switches to Installed tab on click", async () => {
    const wrapper = mountMarketplace();
    await wrapper.find("[data-test='tab-installed']").trigger("click");
    await flushPromises();
    expect(wrapper.find("[data-test='installed-list']").exists()).toBe(true);
  });
});

import { flushPromises } from "@vue/test-utils";

describe("Marketplace.vue — Phase 2 source chips", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("renders a chip per configured source plus a builtin chip", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce([] as never) // list_catalog
      .mockResolvedValueOnce([
        {
          id: "smithery",
          display_name: "Smithery",
          kind: "smithery",
          url: "https://x",
          api_key_env: null,
          priority: 50,
          default_trust: "community",
          enabled: true,
          cache_ttl_seconds: null,
          last_error: null
        }
      ] as never); // list_catalog_sources
    const wrapper = mountMarketplace();
    await flushPromises();
    const chips = wrapper.findAll('[data-test^="source-chip-"]');
    expect(chips.length).toBe(2);
    expect(wrapper.text()).toContain("Built-in");
    expect(wrapper.text()).toContain("Smithery");
  });

  it("shows ⚠ badge when CatalogSourceFailed observed", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce([] as never)
      .mockResolvedValueOnce([
        {
          id: "smithery",
          display_name: "Smithery",
          kind: "smithery",
          url: "https://x",
          api_key_env: null,
          priority: 50,
          default_trust: "community",
          enabled: true,
          cache_ttl_seconds: null,
          last_error: null
        }
      ] as never);
    const wrapper = mountMarketplace();
    await flushPromises();
    useCatalogStore().handleSourceFailed("smithery", "timeout");
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="src-warn-smithery"]').exists()).toBe(true);
  });

  it("deselecting a chip filters its entries out of CatalogList", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce([
        fixtureEntry({ id: "a", source: "builtin", display_name: "A-entry" }),
        fixtureEntry({ id: "b", source: "smithery", display_name: "B-entry" })
      ] as never)
      .mockResolvedValueOnce([
        {
          id: "smithery",
          display_name: "Smithery",
          kind: "smithery",
          url: "https://x",
          api_key_env: null,
          priority: 50,
          default_trust: "community",
          enabled: true,
          cache_ttl_seconds: null,
          last_error: null
        }
      ] as never);
    const wrapper = mountMarketplace();
    await flushPromises();
    expect(wrapper.text()).toContain("A-entry");
    expect(wrapper.text()).toContain("B-entry");
    await wrapper.find('[data-test="source-chip-builtin"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).not.toContain("A-entry");
    expect(wrapper.text()).toContain("B-entry");
  });
});

describe("CatalogCard.vue", () => {
  it("renders display_name, summary, trust, and tags", () => {
    const wrapper = mount(CatalogCard, {
      props: { entry: fixtureEntry() }
    });
    expect(wrapper.text()).toContain("Filesystem");
    expect(wrapper.text()).toContain("Read & write files");
    expect(wrapper.text()).toContain("verified");
    expect(wrapper.text()).toContain("files");
  });

  it("emits click", async () => {
    const wrapper = mount(CatalogCard, {
      props: { entry: fixtureEntry() }
    });
    await wrapper.trigger("click");
    expect(wrapper.emitted("click")).toBeTruthy();
  });
});

describe("RuntimeMissingHint.vue", () => {
  it("renders one item per requirement", () => {
    const wrapper = mount(RuntimeMissingHint, {
      props: {
        requirements: [
          {
            kind: "node",
            min_version: ">=18.0.0",
            install_hint: "https://nodejs.org"
          },
          { kind: "python", min_version: null, install_hint: null }
        ]
      }
    });
    const items = wrapper.findAll("li");
    expect(items.length).toBe(2);
    expect(items[0].text()).toContain("node");
    expect(items[0].text()).toContain(">=18.0.0");
    expect(items[1].text()).toContain("python");
  });
});

describe("InstalledList.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("renders rows for each installed entry", async () => {
    const catalog = useCatalogStore();
    catalog.installed = [
      {
        server_id: "filesystem",
        catalog_id: "filesystem",
        source: "builtin",
        display_name: "Filesystem",
        installed_at: "2026-05-06T00:00:00Z",
        running: true
      }
    ];
    vi.mocked(invoke).mockResolvedValueOnce([]); // refreshInstalled in onMounted
    const wrapper = mount(InstalledList);
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).toContain("Filesystem");
    expect(wrapper.text()).toContain("running");
  });

  it("disables Uninstall for hand-edited (no source) entries", async () => {
    const catalog = useCatalogStore();
    catalog.installed = [
      {
        server_id: "manual-server",
        catalog_id: null,
        source: null,
        display_name: "Manual",
        installed_at: "2026-05-06T00:00:00Z",
        running: false
      }
    ];
    vi.mocked(invoke).mockResolvedValueOnce([]);
    const wrapper = mount(InstalledList);
    await wrapper.vm.$nextTick();
    const btn = wrapper.find("[data-test='uninstall-manual-server']");
    expect(btn.exists()).toBe(true);
    expect(btn.attributes("disabled")).toBeDefined();
  });
});

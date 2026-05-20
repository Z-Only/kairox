import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue([])
}));

import { invoke } from "@tauri-apps/api/core";
import { useCatalogStore } from "@/stores/catalog";
import Marketplace from "../../views/MarketplaceView.vue";
import CatalogCard from "./CatalogCard.vue";
import CatalogDetail from "./CatalogDetail.vue";
import catalogDetailSource from "./CatalogDetail.vue?raw";
import RuntimeMissingHint from "./RuntimeMissingHint.vue";
import InstalledList from "./InstalledList.vue";
import installedListSource from "./InstalledList.vue?raw";

// MarketplaceView calls `useI18n()` so mounting it through plain `mount()`
// would fail with "Need to install with `app.use` function".
// `mountWithPlugins` wires the shared i18n + Pinia + router stack the same
// way every other view test does.
// `reusePinia: true` keeps the Pinia instance created in `beforeEach` so
// that `useCatalogStore()` calls in the test body and inside the component
// reference the same store instance.
function mountMarketplace() {
  const mountOptions: MountWithPluginsOptions<typeof Marketplace> = {
    reusePinia: true,
    initialRoute: "/marketplace"
  };
  return mountWithPlugins(Marketplace, mountOptions).wrapper;
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

  it("renders Browse content without Marketplace-level tabs", async () => {
    const wrapper = mountMarketplace();
    await wrapper.vm.$nextTick();

    expect(wrapper.find("[data-test='catalog-search']").exists()).toBe(true);
    expect(wrapper.find("[data-test='tab-browse']").exists()).toBe(false);
    expect(wrapper.find("[data-test='tab-installed']").exists()).toBe(false);
  });

  it("renders Browse content without the redundant MarketplacePane tab wrapper", async () => {
    const wrapper = mountMarketplace();
    await flushPromises();

    expect(wrapper.find("[data-test='catalog-search']").exists()).toBe(true);
    expect(wrapper.find("[data-test='tab-browse']").exists()).toBe(false);
    expect(wrapper.find("[data-test='tab-installed']").exists()).toBe(false);
    expect(wrapper.find("[data-test='installed-list']").exists()).toBe(false);
  });

  // Task 9 carry-over from Task 8 review NIT-10: lock the existence
  // contract of the `data-test="catalog-trust"` hook. Task 8 deleted the
  // hidden <select> dead code and moved this hook onto the visible
  // NSelect, but no spec asserted it — vitest passing meant "no one tests
  // it", not "the hook is still selectable". This single assertion
  // prevents silent removal in future refactors.
  it("exposes the catalog-trust selector hook on the visible NSelect", async () => {
    const wrapper = mountMarketplace();
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="catalog-trust"]').exists()).toBe(true);
  });
});

import { flushPromises } from "@vue/test-utils";

describe("Marketplace.vue — Phase 2 source chips", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  // Helper: mock invoke by command name rather than call order.
  // MarketplacePane loads sources and catalog entries during mount, and
  // command-name routing keeps these tests stable if the component tree changes.
  // Using `mockResolvedValueOnce` is fragile because the consumption
  // order depends on Vue's component tree walk. This helper routes
  // responses by the first positional argument (the Tauri command name).
  function mockInvokeByCommand(responses: Record<string, unknown>) {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd in responses) return Promise.resolve(responses[cmd]);
      return Promise.resolve([]);
    });
  }

  const mcpRegistrySource = {
    id: "mcp-registry",
    display_name: "Model Context Protocol Servers",
    kind: "mcp_registry",
    url: "https://x",
    api_key_env: null,
    priority: 50,
    default_trust: "community",
    enabled: true,
    cache_ttl_seconds: null,
    last_error: null
  };

  it("renders a chip per configured source plus a builtin chip", async () => {
    mockInvokeByCommand({
      list_catalog_sources: [mcpRegistrySource],
      list_catalog: [],
      list_installed_entries: []
    });
    const wrapper = mountMarketplace();
    await flushPromises();
    const chips = wrapper.findAll('[data-test^="source-chip-"]');
    expect(chips.length).toBe(2);
    expect(wrapper.text()).toContain("Built-in");
    expect(wrapper.text()).toContain("Model Context Protocol Servers");
  });

  it("shows ⚠ badge when CatalogSourceFailed observed", async () => {
    mockInvokeByCommand({
      list_catalog_sources: [mcpRegistrySource],
      list_catalog: [],
      list_installed_entries: []
    });
    const wrapper = mountMarketplace();
    await flushPromises();
    useCatalogStore().handleSourceFailed("mcp-registry", "timeout");
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="src-warn-mcp-registry"]').exists()).toBe(true);
  });

  it("deselecting a chip disables the source and filters its entries", async () => {
    // builtin starts enabled; mcp-registry also enabled=true in the fixture.
    // Clicking builtin calls setSourceEnabled("builtin", false) which is a
    // no-op on the Rust side, but the store's isSourceEnabled() already
    // reflects the source.enabled flag from the fetchSources response.
    // To simulate the toggle we directly mutate the source state as the
    // Rust side would after a successful set_catalog_source_enabled call.
    mockInvokeByCommand({
      list_catalog_sources: [mcpRegistrySource],
      list_catalog: [
        fixtureEntry({ id: "a", source: "builtin", display_name: "A-entry" }),
        fixtureEntry({ id: "b", source: "mcp-registry", display_name: "B-entry" })
      ],
      list_installed_entries: []
    });
    const wrapper = mountMarketplace();
    await flushPromises();
    expect(wrapper.text()).toContain("A-entry");
    expect(wrapper.text()).toContain("B-entry");

    // Simulate set_catalog_source_enabled("mcp-registry", false) + fetchSources
    const store = useCatalogStore();
    store.sources = [
      {
        ...mcpRegistrySource,
        enabled: false
      }
    ];
    await flushPromises();
    expect(wrapper.text()).toContain("A-entry");
    expect(wrapper.text()).not.toContain("B-entry");
  });
});

describe("CatalogCard.vue", () => {
  it("renders display_name, summary, trust, and tags", () => {
    setActivePinia(createPinia());
    const wrapper = mountWithPlugins(CatalogCard, {
      reusePinia: true,
      mount: { props: { entry: fixtureEntry() } }
    }).wrapper;
    expect(wrapper.text()).toContain("Filesystem");
    expect(wrapper.text()).toContain("Read & write files");
    expect(wrapper.text()).toContain("verified");
    expect(wrapper.text()).toContain("files");
  });

  it("emits click", async () => {
    setActivePinia(createPinia());
    const wrapper = mountWithPlugins(CatalogCard, {
      reusePinia: true,
      mount: { props: { entry: fixtureEntry() } }
    }).wrapper;
    await wrapper.find('[data-test="catalog-card"]').trigger("click");
    expect(wrapper.emitted("click")).toBeTruthy();
  });
});

describe("CatalogDetail.vue configuration section", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    document.body.innerHTML = "";
  });

  const mountDetail = (entry = fixtureEntry()) =>
    mountWithPlugins(CatalogDetail, {
      reusePinia: true,
      mount: {
        attachTo: document.body,
        props: { entry }
      }
    }).wrapper;

  it("presents stdio environment configuration as required configuration", async () => {
    const wrapper = mountDetail(
      fixtureEntry({
        display_name: "Filesystem",
        description: "Scoped filesystem access.",
        install_spec_json: JSON.stringify({
          transport: "stdio",
          command: "npx",
          args: ["-y", "@modelcontextprotocol/server-filesystem", "${WORKSPACE_PATH}"],
          env: {},
          cwd: null
        }),
        default_env_json: JSON.stringify([
          {
            key: "WORKSPACE_PATH",
            label: "Workspace path",
            description: "Directory the server is allowed to access.",
            required: true,
            secret: false,
            default: "~"
          }
        ])
      })
    );
    await flushPromises();

    const text = document.body.textContent ?? "";
    expect(text).toContain("Configuration");
    expect(text).toContain("Required configuration");
    expect(text).toContain("Environment variable");
    expect(text).toContain("Workspace path");
    expect(text).toContain("Directory the server is allowed to access.");
    expect(document.body.querySelector('[data-test="config-WORKSPACE_PATH"]')).not.toBeNull();

    wrapper.unmount();
  });

  it("presents streamable HTTP headers with their configuration descriptions", async () => {
    const wrapper = mountDetail(
      fixtureEntry({
        id: "remote-auth",
        source: "mcp-registry",
        display_name: "Remote Auth",
        install_spec_json: JSON.stringify({
          transport: "streamable_http",
          url: "https://example.com/mcp",
          headers: { Authorization: "" }
        }),
        default_env_json: JSON.stringify([
          {
            key: "Authorization",
            label: "Authorization",
            description: "Bearer token from the provider dashboard.",
            required: true,
            secret: true,
            default: null
          }
        ])
      })
    );
    await flushPromises();

    const text = document.body.textContent ?? "";
    expect(text).toContain("Configuration");
    expect(text).toContain("Authentication header");
    expect(text).toContain("Required");
    expect(text).toContain("Bearer token from the provider dashboard.");
    expect(
      document.body.querySelector('[data-test="config-Authorization"]')?.getAttribute("type")
    ).toBe("password");

    wrapper.unmount();
  });

  it("shows a compact no-configuration state when no values are needed", async () => {
    const wrapper = mountDetail(
      fixtureEntry({
        display_name: "Fetch",
        install_spec_json: JSON.stringify({
          transport: "stdio",
          command: "uvx",
          args: ["mcp-server-fetch"],
          env: {},
          cwd: null
        }),
        default_env_json: "[]"
      })
    );
    await flushPromises();

    expect(document.body.textContent ?? "").toContain("No configuration required.");

    wrapper.unmount();
  });

  it("does not keep legacy handcrafted tooltip styles in catalog detail", () => {
    expect(catalogDetailSource).not.toContain("tooltip-wrap");
    expect(catalogDetailSource).not.toContain("tooltip-active");
    expect(catalogDetailSource).not.toContain("data-tooltip");
  });

  it("does not keep shared catalog detail chrome copy inline in the component source", () => {
    expect(catalogDetailSource).not.toMatch(/>\s*Homepage\s*</);
    expect(catalogDetailSource).not.toMatch(/>\s*Requirements\s*</);
    expect(catalogDetailSource).not.toMatch(/>\s*Configuration\s*</);
    expect(catalogDetailSource).not.toMatch(/>\s*Required configuration\s*</);
    expect(catalogDetailSource).not.toMatch(/>\s*No configuration required\.\s*</);
    expect(catalogDetailSource).not.toMatch(/>\s*No description provided by the catalog\.\s*</);
    expect(catalogDetailSource).not.toMatch(/>\s*Trust this server/);
    expect(catalogDetailSource).not.toMatch(/>\s*Start after install\s*</);
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

  // `InstalledList.vue` calls `useI18n()` (Task 9 i18n sweep) so bare
  // `mount()` throws "Need to install with `app.use` function". We use
  // `mountWithPlugins` to install i18n + router; `reusePinia: true`
  // keeps the `beforeEach` pinia alive so `catalog.installed = [...]`
  // mutations done in each test before mounting are not wiped. Returns
  // the bare wrapper so the existing `wrapper.find` / `wrapper.text`
  // call-sites stay drop-in compatible.
  const mountInstalled = () => {
    const mountOptions: MountWithPluginsOptions<typeof InstalledList> = { reusePinia: true };
    return mountWithPlugins(InstalledList, mountOptions).wrapper;
  };

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
    const wrapper = mountInstalled();
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
    const wrapper = mountInstalled();
    await wrapper.vm.$nextTick();
    const btn = wrapper.find("[data-test='uninstall-manual-server']");
    expect(btn.exists()).toBe(true);
    expect(btn.attributes("disabled")).toBeDefined();
  });

  it("does not keep installed-list table and action copy inline in the component source", () => {
    expect(installedListSource).not.toMatch(/<th>\s*Server\s*<\/th>/);
    expect(installedListSource).not.toMatch(/<th>\s*Source\s*<\/th>/);
    expect(installedListSource).not.toMatch(/<th>\s*Status\s*<\/th>/);
    expect(installedListSource).not.toMatch(/<th>\s*Installed at\s*<\/th>/);
    expect(installedListSource).not.toContain('?? "(manual)"');
    expect(installedListSource).not.toContain('? "running"');
    expect(installedListSource).not.toContain(': "stopped"');
    expect(installedListSource).not.toContain("Hand-edited entries are not removable from here");
    expect(installedListSource).not.toMatch(/>\s*Uninstall\s*</);
  });

  it("audit anchors: exposes stable marketplace view pilot selector", async () => {
    const wrapper = mountMarketplace();
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="view-marketplace"]').exists()).toBe(true);
  });
});

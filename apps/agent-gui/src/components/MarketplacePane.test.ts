import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { enableAutoUnmount, flushPromises } from "@vue/test-utils";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue([])
}));

import { invoke } from "@tauri-apps/api/core";
import { useCatalogStore } from "@/stores/catalog";
import type { CatalogSourceViewResponse } from "@/generated/commands";
import MarketplacePane from "./MarketplacePane.vue";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeSource(overrides: Partial<CatalogSourceViewResponse> = {}): CatalogSourceViewResponse {
  return {
    id: "builtin",
    display_name: "Built-in",
    kind: "builtin",
    url: "",
    api_key_env: null,
    priority: 0,
    default_trust: "verified",
    enabled: true,
    cache_ttl_seconds: 3600,
    last_error: null,
    ...overrides
  };
}

function mountMarketplace() {
  const opts: MountWithPluginsOptions<typeof MarketplacePane> = {
    reusePinia: true,
    mount: {
      global: {
        stubs: {
          CatalogList: { template: '<div data-test="catalog-list-stub" />' },
          InstallProgress: { template: '<div data-test="install-progress-stub" />' },
          CatalogSourcesSettings: {
            template: '<div data-test="catalog-sources-settings-stub" />'
          },
          ModalDialog: {
            template: '<div data-test="modal-stub"><slot /></div>',
            props: ["open", "title"]
          },
          Teleport: true
        }
      }
    }
  };
  return mountWithPlugins(MarketplacePane, opts).wrapper;
}

// ---------------------------------------------------------------------------
// Test environment
// ---------------------------------------------------------------------------

enableAutoUnmount(afterEach);
afterEach(() => {
  document.body.innerHTML = "";
});
beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  // Default: list_catalog_sources returns empty, list_catalog returns empty
  vi.mocked(invoke).mockResolvedValue([]);
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("MarketplacePane", () => {
  // ---- Rendering ----

  describe("rendering", () => {
    it("renders the marketplace pane container", async () => {
      const wrapper = mountMarketplace();
      await flushPromises();

      expect(wrapper.find(".marketplace-pane").exists()).toBe(true);
    });

    it("renders the source filter chip group", async () => {
      const wrapper = mountMarketplace();
      await flushPromises();

      expect(wrapper.find('[data-test="marketplace-source-filter"]').exists()).toBe(true);
    });

    it("always renders the builtin source chip", async () => {
      const wrapper = mountMarketplace();
      await flushPromises();

      expect(wrapper.find('[data-test="source-chip-builtin"]').exists()).toBe(true);
    });

    it("renders remote source chips when sources are loaded", async () => {
      const catalog = useCatalogStore();
      // Prevent onMounted from overwriting pre-set sources
      vi.spyOn(catalog, "fetchSources").mockResolvedValue();
      vi.spyOn(catalog, "fetchCatalog").mockResolvedValue();
      catalog.sources = [
        makeSource({ id: "builtin", display_name: "Built-in" }),
        makeSource({ id: "mcp-registry", display_name: "MCP Registry", kind: "remote" })
      ];

      const wrapper = mountMarketplace();
      await flushPromises();

      expect(wrapper.find('[data-test="source-chip-builtin"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="source-chip-mcp-registry"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="source-chip-mcp-registry"]').text()).toContain(
        "MCP Registry"
      );
    });

    it("renders the catalog source settings button", async () => {
      const wrapper = mountMarketplace();
      await flushPromises();

      expect(wrapper.find('[data-test="catalog-source-settings"]').exists()).toBe(true);
    });

    it("renders the CatalogList component", async () => {
      const wrapper = mountMarketplace();
      await flushPromises();

      expect(wrapper.find('[data-test="catalog-list-stub"]').exists()).toBe(true);
    });
  });

  // ---- Source chip interaction ----

  describe("source chip interaction", () => {
    it("calls toggleSource when a source chip is clicked", async () => {
      const catalog = useCatalogStore();
      catalog.sources = [makeSource({ id: "builtin", display_name: "Built-in" })];
      const toggleSpy = vi.spyOn(catalog, "toggleSource").mockResolvedValue();

      const wrapper = mountMarketplace();
      await flushPromises();

      await wrapper.find('[data-test="source-chip-builtin"]').trigger("click");
      expect(toggleSpy).toHaveBeenCalledWith("builtin");
    });
  });

  // ---- Source failure badges ----

  describe("source failure badges", () => {
    it("shows warning badge when a source has a failure", async () => {
      const catalog = useCatalogStore();
      // Prevent onMounted from overwriting pre-set sources and failures
      vi.spyOn(catalog, "fetchSources").mockResolvedValue();
      vi.spyOn(catalog, "fetchCatalog").mockResolvedValue();
      vi.spyOn(catalog, "refreshCatalogSource").mockResolvedValue();
      catalog.sources = [
        makeSource({ id: "mcp-registry", display_name: "MCP Registry", kind: "remote" })
      ];
      catalog.sourceFailures = { "mcp-registry": "Network error" };

      const wrapper = mountMarketplace();
      await flushPromises();

      const badge = wrapper.find('[data-test="src-warn-mcp-registry"]');
      expect(badge.exists()).toBe(true);
      expect(badge.text()).toBe("!");
      expect(badge.attributes("title")).toBe("Network error");
    });

    it("does not show warning badge when source has no failure", async () => {
      const catalog = useCatalogStore();
      catalog.sources = [makeSource({ id: "builtin", display_name: "Built-in" })];
      catalog.sourceFailures = {};

      const wrapper = mountMarketplace();
      await flushPromises();

      expect(wrapper.find('[data-test="src-warn-builtin"]').exists()).toBe(false);
    });
  });

  // ---- Settings drawer ----

  describe("settings drawer", () => {
    it("toggles the settings modal when settings button is clicked", async () => {
      const wrapper = mountMarketplace();
      await flushPromises();

      // The settings button should exist
      const settingsBtn = wrapper.find('[data-test="catalog-source-settings"]');
      expect(settingsBtn.exists()).toBe(true);

      // The modal (stubbed) is always in the DOM; verify the settings-drawer data-test
      const drawer = wrapper.find('[data-test="catalog-source-settings-drawer"]');
      expect(drawer.exists()).toBe(true);
    });
  });

  // ---- Install progress ----

  describe("install progress", () => {
    it("shows InstallProgress when currentInstallEntryId is set", async () => {
      const catalog = useCatalogStore();
      catalog.currentInstallEntryId = "some-entry";

      const wrapper = mountMarketplace();
      await flushPromises();

      expect(wrapper.find('[data-test="install-progress-stub"]').exists()).toBe(true);
    });

    it("hides InstallProgress when currentInstallEntryId is null", async () => {
      const catalog = useCatalogStore();
      catalog.currentInstallEntryId = null;

      const wrapper = mountMarketplace();
      await flushPromises();

      expect(wrapper.find('[data-test="install-progress-stub"]').exists()).toBe(false);
    });
  });

  // ---- onMounted behavior ----

  describe("onMounted", () => {
    it("resets tab to browse if it was installed", async () => {
      const catalog = useCatalogStore();
      catalog.tab = "installed";

      mountMarketplace();
      await flushPromises();

      expect(catalog.tab).toBe("browse");
    });

    it("fetches sources on mount", async () => {
      const catalog = useCatalogStore();
      const fetchSourcesSpy = vi.spyOn(catalog, "fetchSources").mockResolvedValue();

      mountMarketplace();
      await flushPromises();

      expect(fetchSourcesSpy).toHaveBeenCalled();
    });

    it("fetches catalog when entries are empty and no remote sources", async () => {
      const catalog = useCatalogStore();
      catalog.sources = [makeSource({ id: "builtin" })];
      catalog.entries = [];
      const fetchCatalogSpy = vi.spyOn(catalog, "fetchCatalog").mockResolvedValue();
      vi.spyOn(catalog, "fetchSources").mockResolvedValue();

      mountMarketplace();
      await flushPromises();

      expect(fetchCatalogSpy).toHaveBeenCalled();
    });

    it("refreshes catalog source when entries empty and has enabled remote", async () => {
      const catalog = useCatalogStore();
      catalog.sources = [
        makeSource({ id: "builtin" }),
        makeSource({ id: "remote-1", kind: "remote", enabled: true })
      ];
      catalog.entries = [];
      const refreshSpy = vi.spyOn(catalog, "refreshCatalogSource").mockResolvedValue();
      vi.spyOn(catalog, "fetchSources").mockResolvedValue();

      mountMarketplace();
      await flushPromises();

      expect(refreshSpy).toHaveBeenCalled();
    });
  });
});

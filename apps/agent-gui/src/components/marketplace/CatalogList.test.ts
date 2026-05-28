import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { flushPromises } from "@vue/test-utils";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";
import type { ServerEntryResponse } from "../../generated/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue([])
}));

import { invoke } from "@tauri-apps/api/core";
import { useCatalogStore } from "@/stores/catalog";
import CatalogList from "./CatalogList.vue";

// ── fixture helpers ──────────────────────────────────────────────────

const fixtureEntry = (over: Partial<ServerEntryResponse> = {}): ServerEntryResponse => ({
  id: "test-server",
  source: "builtin",
  display_name: "Test Server",
  summary: "A test MCP server",
  description: "Full description.",
  categories: ["testing"],
  tags: ["test"],
  author: null,
  homepage: null,
  version: null,
  trust: "verified",
  verified: true,
  icon: null,
  install_spec_json: "{}",
  requirements_json: "[]",
  default_env_json: "[]",
  ...over
});

function mountCatalogList() {
  const opts: MountWithPluginsOptions<typeof CatalogList> = { reusePinia: true };
  return mountWithPlugins(CatalogList, opts).wrapper;
}

// ── tests ────────────────────────────────────────────────────────────

describe("CatalogList.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  // ── 1. Render smoke ──

  describe("render smoke", () => {
    it("renders filter bar with search, trust, and sort controls", async () => {
      const wrapper = mountCatalogList();
      await flushPromises();

      expect(wrapper.find('[data-test="catalog-search"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="catalog-trust"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="catalog-sort"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="catalog-refresh"]').exists()).toBe(true);
    });

    it("renders catalog cards for each entry", async () => {
      const catalog = useCatalogStore();
      catalog.entries = [
        fixtureEntry({ id: "a", display_name: "Alpha" }),
        fixtureEntry({ id: "b", display_name: "Beta" })
      ];
      const wrapper = mountCatalogList();
      await flushPromises();

      const cards = wrapper.findAll('[data-test="catalog-card"]');
      expect(cards).toHaveLength(2);
      expect(wrapper.text()).toContain("Alpha");
      expect(wrapper.text()).toContain("Beta");
    });
  });

  // ── 2. Empty / loading / error states ──

  describe("state displays", () => {
    it("shows loading state when catalog is loading", async () => {
      // Seed entries so onMounted doesn't trigger fetchCatalog
      const catalog = useCatalogStore();
      catalog.entries = [fixtureEntry()];
      const wrapper = mountCatalogList();
      await flushPromises();

      // Now set loading after mount to avoid onMounted resetting it
      catalog.loading = true;
      await wrapper.vm.$nextTick();

      expect(wrapper.find('[data-test="catalog-loading-state"]').exists()).toBe(true);
    });

    it("shows error state when catalog has an error", async () => {
      // Seed entries so onMounted doesn't trigger fetchCatalog
      const catalog = useCatalogStore();
      catalog.entries = [fixtureEntry()];
      const wrapper = mountCatalogList();
      await flushPromises();

      // Set error after mount
      catalog.error = "Network failure";
      await wrapper.vm.$nextTick();

      expect(wrapper.find('[data-test="catalog-error-state"]').exists()).toBe(true);
      expect(wrapper.text()).toContain("Network failure");
    });

    it("shows empty state when no entries match filters", async () => {
      const catalog = useCatalogStore();
      catalog.entries = [];
      const wrapper = mountCatalogList();
      await flushPromises();

      expect(wrapper.find('[data-test="catalog-empty-state"]').exists()).toBe(true);
    });
  });

  // ── 3. Search filtering ──

  describe("search filtering", () => {
    it("filters cards as the search input changes", async () => {
      const catalog = useCatalogStore();
      catalog.entries = [
        fixtureEntry({ id: "fs", display_name: "Filesystem", summary: "Read files" }),
        fixtureEntry({ id: "web", display_name: "Web Fetch", summary: "HTTP" })
      ];
      const wrapper = mountCatalogList();
      await flushPromises();

      expect(wrapper.findAll('[data-test="catalog-card"]')).toHaveLength(2);

      await wrapper.find('[data-test="catalog-search"]').setValue("filesystem");
      await wrapper.vm.$nextTick();

      expect(wrapper.findAll('[data-test="catalog-card"]')).toHaveLength(1);
      expect(wrapper.text()).toContain("Filesystem");
      expect(wrapper.text()).not.toContain("Web Fetch");
    });

    it("hydrates search from store keyword filter", async () => {
      const catalog = useCatalogStore();
      catalog.entries = [
        fixtureEntry({ id: "fs", display_name: "Filesystem", summary: "Files" }),
        fixtureEntry({ id: "web", display_name: "Web Fetch", summary: "HTTP" })
      ];
      catalog.filters.keyword = "web";
      const wrapper = mountCatalogList();
      await flushPromises();

      const input = wrapper.find('[data-test="catalog-search"]').element as HTMLInputElement;
      expect(input.value).toBe("web");
      expect(wrapper.findAll('[data-test="catalog-card"]')).toHaveLength(1);
    });
  });

  // ── 4. Sorting ──

  describe("sorting", () => {
    it("sorts cards by name by default (alphabetical)", async () => {
      const catalog = useCatalogStore();
      catalog.entries = [
        fixtureEntry({ id: "z", display_name: "Zeta", trust: "community" }),
        fixtureEntry({ id: "a", display_name: "Alpha", trust: "verified" })
      ];
      const wrapper = mountCatalogList();
      await flushPromises();

      const names = wrapper
        .findAll('[data-test="catalog-card"] .display-name')
        .map((el) => el.text());
      expect(names).toEqual(["Alpha", "Zeta"]);
    });

    it("sorts by trust descending when trust sort is selected", async () => {
      const catalog = useCatalogStore();
      catalog.entries = [
        fixtureEntry({ id: "a", display_name: "Alpha", trust: "community" }),
        fixtureEntry({ id: "b", display_name: "Beta", trust: "verified" })
      ];
      const wrapper = mountCatalogList();
      await flushPromises();

      await wrapper.find('[data-test="catalog-sort"]').setValue("trust");
      await wrapper.vm.$nextTick();

      const names = wrapper
        .findAll('[data-test="catalog-card"] .display-name')
        .map((el) => el.text());
      expect(names).toEqual(["Beta", "Alpha"]);
    });

    it("sorts by source when source sort is selected", async () => {
      const catalog = useCatalogStore();
      catalog.entries = [
        fixtureEntry({ id: "b", display_name: "Beta", source: "mcp-registry" }),
        fixtureEntry({ id: "a", display_name: "Alpha", source: "builtin" })
      ];
      // Register the mcp-registry source so visibleEntries includes it
      catalog.sources = [
        {
          id: "mcp-registry",
          display_name: "MCP Registry",
          kind: "mcp_registry",
          url: "https://registry.example.test",
          api_key_env: null,
          priority: 10,
          default_trust: "community",
          enabled: true,
          cache_ttl_seconds: null,
          last_error: null
        }
      ];
      const wrapper = mountCatalogList();
      await flushPromises();

      await wrapper.find('[data-test="catalog-sort"]').setValue("source");
      await wrapper.vm.$nextTick();

      const names = wrapper
        .findAll('[data-test="catalog-card"] .display-name')
        .map((el) => el.text());
      expect(names).toEqual(["Alpha", "Beta"]);
    });
  });

  // ── 5. Refresh action ──

  describe("refresh action", () => {
    it("calls fetchCatalog on refresh button click", async () => {
      const catalog = useCatalogStore();
      catalog.entries = [fixtureEntry()];
      const wrapper = mountCatalogList();
      await flushPromises();
      vi.clearAllMocks();

      await wrapper.find('[data-test="catalog-refresh"]').trigger("click");
      await flushPromises();

      expect(invoke).toHaveBeenCalled();
    });
  });
});

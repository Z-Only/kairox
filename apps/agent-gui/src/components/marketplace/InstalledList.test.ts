import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { flushPromises } from "@vue/test-utils";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue([])
}));

import { invoke } from "@tauri-apps/api/core";
import { useCatalogStore } from "@/stores/catalog";
import InstalledList from "./InstalledList.vue";

// ── helpers ──────────────────────────────────────────────────────────

function mountInstalled() {
  const opts: MountWithPluginsOptions<typeof InstalledList> = { reusePinia: true };
  return mountWithPlugins(InstalledList, opts).wrapper;
}

// ── tests ────────────────────────────────────────────────────────────

describe("InstalledList.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  // ── 1. Render smoke ──

  describe("render smoke", () => {
    it("renders the installed-list wrapper", async () => {
      vi.mocked(invoke).mockResolvedValueOnce([]);
      const wrapper = mountInstalled();
      await flushPromises();

      expect(wrapper.find('[data-test="installed-list"]').exists()).toBe(true);
    });

    it("renders rows for each installed entry", async () => {
      const catalog = useCatalogStore();
      catalog.installed = [
        {
          server_id: "filesystem",
          catalog_id: "filesystem",
          source: "builtin",
          display_name: "Filesystem",
          installed_at: "2026-05-28T00:00:00Z",
          running: true
        },
        {
          server_id: "web-fetch",
          catalog_id: "web-fetch",
          source: "mcp-registry",
          display_name: "Web Fetch",
          installed_at: "2026-05-27T00:00:00Z",
          running: false
        }
      ];
      vi.mocked(invoke).mockResolvedValueOnce([]);
      const wrapper = mountInstalled();
      await wrapper.vm.$nextTick();

      expect(wrapper.text()).toContain("Filesystem");
      expect(wrapper.text()).toContain("Web Fetch");
    });

    it("shows running status for active entries", async () => {
      const catalog = useCatalogStore();
      catalog.installed = [
        {
          server_id: "fs",
          catalog_id: "fs",
          source: "builtin",
          display_name: "FS",
          installed_at: "2026-05-28T00:00:00Z",
          running: true
        }
      ];
      vi.mocked(invoke).mockResolvedValueOnce([]);
      const wrapper = mountInstalled();
      await wrapper.vm.$nextTick();

      expect(wrapper.text()).toMatch(/running/i);
    });

    it("shows stopped status for inactive entries", async () => {
      const catalog = useCatalogStore();
      catalog.installed = [
        {
          server_id: "fs",
          catalog_id: "fs",
          source: "builtin",
          display_name: "FS",
          installed_at: "2026-05-28T00:00:00Z",
          running: false
        }
      ];
      vi.mocked(invoke).mockResolvedValueOnce([]);
      const wrapper = mountInstalled();
      await wrapper.vm.$nextTick();

      expect(wrapper.text()).toMatch(/stopped/i);
    });
  });

  // ── 2. Empty state ──

  describe("empty state", () => {
    it("shows empty state when no entries are installed", async () => {
      vi.mocked(invoke).mockResolvedValueOnce([]);
      const wrapper = mountInstalled();
      await flushPromises();

      expect(wrapper.find('[data-test="installed-empty-state"]').exists()).toBe(true);
    });
  });

  // ── 3. Uninstall button ──

  describe("uninstall button", () => {
    it("enables uninstall button for catalog-sourced entries", async () => {
      const catalog = useCatalogStore();
      catalog.installed = [
        {
          server_id: "fs-server",
          catalog_id: "fs-server",
          source: "builtin",
          display_name: "FS",
          installed_at: "2026-05-28T00:00:00Z",
          running: true
        }
      ];
      vi.mocked(invoke).mockResolvedValueOnce([]);
      const wrapper = mountInstalled();
      await wrapper.vm.$nextTick();

      const btn = wrapper.find('[data-test="uninstall-fs-server"]');
      expect(btn.exists()).toBe(true);
      expect(btn.attributes("disabled")).toBeUndefined();
    });

    it("disables uninstall for hand-edited (no source) entries", async () => {
      const catalog = useCatalogStore();
      catalog.installed = [
        {
          server_id: "manual-server",
          catalog_id: null,
          source: null,
          display_name: "Manual",
          installed_at: "2026-05-28T00:00:00Z",
          running: false
        }
      ];
      vi.mocked(invoke).mockResolvedValueOnce([]);
      const wrapper = mountInstalled();
      await wrapper.vm.$nextTick();

      const btn = wrapper.find('[data-test="uninstall-manual-server"]');
      expect(btn.exists()).toBe(true);
      expect(btn.attributes("disabled")).toBeDefined();
    });

    it("calls uninstallEntry on uninstall click", async () => {
      const catalog = useCatalogStore();
      catalog.installed = [
        {
          server_id: "fs-server",
          catalog_id: "fs-server",
          source: "builtin",
          display_name: "FS",
          installed_at: "2026-05-28T00:00:00Z",
          running: true
        }
      ];
      vi.mocked(invoke).mockResolvedValue([]);
      const wrapper = mountInstalled();
      await wrapper.vm.$nextTick();

      await wrapper.find('[data-test="uninstall-fs-server"]').trigger("click");
      await flushPromises();

      expect(invoke).toHaveBeenCalledWith("uninstall_catalog_entry", {
        serverId: "fs-server"
      });
    });
  });

  // ── 4. Source display ──

  describe("source display", () => {
    it("shows source label for catalog entries", async () => {
      const catalog = useCatalogStore();
      catalog.installed = [
        {
          server_id: "fs",
          catalog_id: "fs",
          source: "builtin",
          display_name: "FS",
          installed_at: "2026-05-28T00:00:00Z",
          running: true
        }
      ];
      vi.mocked(invoke).mockResolvedValueOnce([]);
      const wrapper = mountInstalled();
      await wrapper.vm.$nextTick();

      expect(wrapper.text()).toContain("builtin");
    });

    it("shows manual source label for entries without source", async () => {
      const catalog = useCatalogStore();
      catalog.installed = [
        {
          server_id: "manual",
          catalog_id: null,
          source: null,
          display_name: "Manual",
          installed_at: "2026-05-28T00:00:00Z",
          running: false
        }
      ];
      vi.mocked(invoke).mockResolvedValueOnce([]);
      const wrapper = mountInstalled();
      await wrapper.vm.$nextTick();

      // The i18n key marketplace.installedList.manualSource renders "(manual)"
      expect(wrapper.text()).toMatch(/manual/i);
    });
  });

  // ── 5. Fetches installed on mount ──

  describe("lifecycle", () => {
    it("calls fetchInstalled on mount", async () => {
      vi.mocked(invoke).mockResolvedValueOnce([]);
      mountInstalled();
      await flushPromises();

      expect(invoke).toHaveBeenCalledWith("list_installed_entries");
    });
  });
});

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
import CatalogCard from "./CatalogCard.vue";

// ── fixture helpers ──────────────────────────────────────────────────

const fixtureEntry = (over: Partial<ServerEntryResponse> = {}): ServerEntryResponse => ({
  id: "test-server",
  source: "builtin",
  display_name: "Test Server",
  summary: "A test MCP server",
  description: "Full description of the test server.",
  categories: ["testing"],
  tags: ["test", "mcp"],
  author: "kairox-team",
  homepage: "https://example.com",
  version: "1.0.0",
  trust: "verified",
  verified: true,
  icon: "🧪",
  install_spec_json: JSON.stringify({
    transport: "stdio",
    command: "npx",
    args: ["-y", "test-server"],
    env: {},
    cwd: null
  }),
  requirements_json: "[]",
  default_env_json: "[]",
  ...over
});

function mountCard(entry = fixtureEntry()) {
  const opts: MountWithPluginsOptions<typeof CatalogCard> = {
    reusePinia: true,
    mount: { props: { entry } }
  };
  return mountWithPlugins(CatalogCard, opts).wrapper;
}

// ── tests ────────────────────────────────────────────────────────────

describe("CatalogCard.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  // ── 1. Render smoke ──

  describe("render smoke", () => {
    it("renders display_name, summary, trust badge, and tags", () => {
      const wrapper = mountCard();
      expect(wrapper.text()).toContain("Test Server");
      expect(wrapper.text()).toContain("A test MCP server");
      expect(wrapper.text()).toContain("verified");
      expect(wrapper.text()).toContain("test");
      expect(wrapper.text()).toContain("mcp");
    });

    it("renders the icon from the entry", () => {
      const wrapper = mountCard(fixtureEntry({ icon: "📁" }));
      expect(wrapper.text()).toContain("📁");
    });

    it("renders fallback icon when icon is null", () => {
      const wrapper = mountCard(fixtureEntry({ icon: null }));
      expect(wrapper.text()).toContain("🔌");
    });
  });

  // ── 2. Click interaction ──

  describe("click interaction", () => {
    it("emits click when the card body is clicked", async () => {
      const wrapper = mountCard();
      await wrapper.find('[data-test="catalog-card"]').trigger("click");
      expect(wrapper.emitted("click")).toBeTruthy();
      expect(wrapper.emitted("click")!.length).toBe(1);
    });
  });

  // ── 3. Install button ──

  describe("install button", () => {
    it("shows install button when entry is not installed", () => {
      const wrapper = mountCard();
      const btn = wrapper.find('[data-test="catalog-card-install"]');
      expect(btn.exists()).toBe(true);
      expect(btn.attributes("disabled")).toBeUndefined();
    });

    it("shows installed badge when entry is already installed", () => {
      const catalog = useCatalogStore();
      catalog.installed = [
        {
          server_id: "test-server",
          catalog_id: "test-server",
          source: "builtin",
          display_name: "Test Server",
          installed_at: "2026-05-28T00:00:00Z",
          running: true
        }
      ];
      const wrapper = mountCard();
      expect(wrapper.find('[data-test="catalog-card-install"]').exists()).toBe(false);
      expect(wrapper.text()).toContain("Installed");
    });

    it("disables install button when another entry is being installed", () => {
      const catalog = useCatalogStore();
      catalog.currentInstallEntryId = "other-entry";
      const wrapper = mountCard();
      const btn = wrapper.find('[data-test="catalog-card-install"]');
      expect(btn.exists()).toBe(true);
      expect(btn.attributes("disabled")).toBeDefined();
    });

    it("does not disable install button when the same entry is being installed", () => {
      const catalog = useCatalogStore();
      catalog.currentInstallEntryId = "test-server";
      const wrapper = mountCard();
      const btn = wrapper.find('[data-test="catalog-card-install"]');
      expect(btn.attributes("disabled")).toBeUndefined();
    });

    it("calls installEntry on install button click", async () => {
      vi.mocked(invoke).mockResolvedValue({ kind: "installed", server_id: "test-server" });
      const wrapper = mountCard();
      await wrapper.find('[data-test="catalog-card-install"]').trigger("click");
      await flushPromises();
      expect(invoke).toHaveBeenCalledWith("install_catalog_entry", {
        request: expect.objectContaining({
          catalog_id: "test-server",
          source: "builtin",
          trust_grant: false,
          auto_start: true
        })
      });
    });

    it("install click does not propagate to card click", async () => {
      vi.mocked(invoke).mockResolvedValue({ kind: "installed", server_id: "test-server" });
      const wrapper = mountCard();
      await wrapper.find('[data-test="catalog-card-install"]').trigger("click");
      await flushPromises();
      // click event should NOT have been emitted because .stop modifier
      expect(wrapper.emitted("click")).toBeUndefined();
    });
  });

  // ── 4. Trust badge tone ──

  describe("trust badge tone", () => {
    it("renders success tone for verified trust", () => {
      const wrapper = mountCard(fixtureEntry({ trust: "verified" }));
      expect(wrapper.text()).toContain("verified");
    });

    it("renders warning tone for community trust", () => {
      const wrapper = mountCard(fixtureEntry({ trust: "community" }));
      expect(wrapper.text()).toContain("community");
    });

    it("renders unverified trust label", () => {
      const wrapper = mountCard(fixtureEntry({ trust: "unverified" }));
      expect(wrapper.text()).toContain("unverified");
    });
  });
});

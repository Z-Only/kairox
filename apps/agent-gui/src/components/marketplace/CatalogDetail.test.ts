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
import { useMcpStore } from "@/stores/mcp";
import CatalogDetail from "./CatalogDetail.vue";

// ── fixture helpers ──────────────────────────────────────────────────

const fixtureEntry = (over: Partial<ServerEntryResponse> = {}): ServerEntryResponse => ({
  id: "test-server",
  source: "builtin",
  display_name: "Test Server",
  summary: "A test MCP server",
  description: "Full description of the test server.",
  categories: ["testing"],
  tags: ["test"],
  author: "kairox-team",
  homepage: "https://example.com",
  version: "1.2.0",
  trust: "verified",
  verified: true,
  icon: null,
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

function seedInstalled(catalogId: string, serverId = catalogId) {
  const catalog = useCatalogStore();
  catalog.installed = [
    {
      server_id: serverId,
      catalog_id: catalogId,
      source: "builtin",
      display_name: "Test Server",
      installed_at: "2026-05-28T00:00:00Z",
      running: true
    }
  ];
  useMcpStore().effectiveServers = [
    {
      value: {
        id: serverId,
        name: "Test Server",
        transport: "stdio",
        enabled: true,
        runtime_status: "running",
        trusted: false,
        tool_count: 5,
        last_error: null,
        writable: true,
        config_path: "/tmp/kairox.toml",
        description: "Test"
      },
      source: "User",
      overrides: null,
      enabled: true,
      disabledBy: null,
      writable: true,
      deletable: true
    }
  ];
}

// Mount CatalogDetail with the full plugin stack. Attaches to
// `document.body` so that the KxDrawer teleport renders into a
// queryable DOM tree.
function mountDetail(entry = fixtureEntry()) {
  const opts: MountWithPluginsOptions<typeof CatalogDetail> = {
    reusePinia: true,
    mount: {
      attachTo: document.body,
      props: { entry }
    }
  };
  return mountWithPlugins(CatalogDetail, opts).wrapper;
}

// ── tests ────────────────────────────────────────────────────────────

describe("CatalogDetail.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    document.body.innerHTML = "";
  });

  // ── 1. Rendering catalog entry details ──

  describe("renders catalog entry details", () => {
    it("shows display_name, description, and homepage link", async () => {
      const wrapper = mountDetail();
      await flushPromises();

      const text = document.body.textContent ?? "";
      expect(text).toContain("Test Server");
      expect(text).toContain("Full description of the test server.");

      const homepageLink = document.body.querySelector('a[href="https://example.com"]');
      expect(homepageLink).not.toBeNull();
      expect(homepageLink?.getAttribute("target")).toBe("_blank");
      expect(homepageLink?.getAttribute("rel")).toContain("noopener");

      wrapper.unmount();
    });

    it("hides homepage link when homepage is null", async () => {
      const wrapper = mountDetail(fixtureEntry({ homepage: null }));
      await flushPromises();

      const links = document.body.querySelectorAll("a[target='_blank']");
      expect(links.length).toBe(0);

      wrapper.unmount();
    });

    it("renders the drawer title from display_name", async () => {
      const wrapper = mountDetail(fixtureEntry({ display_name: "Custom Name" }));
      await flushPromises();

      // KxDrawer receives display_name as :title prop
      expect(document.body.textContent).toContain("Custom Name");

      wrapper.unmount();
    });
  });

  // ── 2. Install button state ──

  describe("install button state", () => {
    it("shows install button when entry is not installed", async () => {
      const wrapper = mountDetail();
      await flushPromises();

      const installBtn = document.body.querySelector<HTMLButtonElement>(
        '[data-test="catalog-install"]'
      );
      expect(installBtn).not.toBeNull();
      expect(installBtn?.disabled).toBe(false);

      wrapper.unmount();
    });

    it("hides install button when entry is already installed", async () => {
      seedInstalled("test-server");

      const wrapper = mountDetail();
      await flushPromises();

      const installBtn = document.body.querySelector('[data-test="catalog-install"]');
      expect(installBtn).toBeNull();

      wrapper.unmount();
    });

    it("disables install button when another entry is being installed", async () => {
      const catalog = useCatalogStore();
      catalog.currentInstallEntryId = "other-entry";

      const wrapper = mountDetail();
      await flushPromises();

      const installBtn = document.body.querySelector<HTMLButtonElement>(
        '[data-test="catalog-install"]'
      );
      expect(installBtn).not.toBeNull();
      expect(installBtn?.disabled).toBe(true);

      wrapper.unmount();
    });

    it("does not disable install button when the same entry is being installed", async () => {
      const catalog = useCatalogStore();
      catalog.currentInstallEntryId = "test-server";

      const wrapper = mountDetail();
      await flushPromises();

      const installBtn = document.body.querySelector<HTMLButtonElement>(
        '[data-test="catalog-install"]'
      );
      expect(installBtn).not.toBeNull();
      expect(installBtn?.disabled).toBe(false);

      wrapper.unmount();
    });
  });

  // ── 3. Install action ──

  describe("install action", () => {
    it("calls installEntry with correct payload on install click", async () => {
      vi.mocked(invoke).mockResolvedValue({ kind: "installed", server_id: "test-server" });

      const wrapper = mountDetail();
      await flushPromises();

      const installBtn = document.body.querySelector<HTMLButtonElement>(
        '[data-test="catalog-install"]'
      );
      installBtn?.click();
      await flushPromises();

      expect(invoke).toHaveBeenCalledWith("install_catalog_entry", {
        request: expect.objectContaining({
          catalog_id: "test-server",
          source: "builtin",
          trust_grant: false,
          auto_start: true
        })
      });

      wrapper.unmount();
    });
  });

  // ── 4. Installed status and connectivity ──

  describe("installed status display", () => {
    it("shows installed status badge when entry is installed", async () => {
      seedInstalled("test-server");

      const wrapper = mountDetail();
      await flushPromises();

      const status = document.body.querySelector('[data-test="catalog-installed-status"]');
      expect(status).not.toBeNull();
      expect(status?.textContent).toContain("Installed");

      wrapper.unmount();
    });

    it("hides installed status badge when not installed", async () => {
      const wrapper = mountDetail();
      await flushPromises();

      const status = document.body.querySelector('[data-test="catalog-installed-status"]');
      expect(status).toBeNull();

      wrapper.unmount();
    });

    it("shows test connectivity button for installed entries", async () => {
      seedInstalled("test-server");

      const wrapper = mountDetail();
      await flushPromises();

      const testBtn = document.body.querySelector<HTMLButtonElement>(
        '[data-test="catalog-test-connectivity"]'
      );
      expect(testBtn).not.toBeNull();

      wrapper.unmount();
    });

    it("hides test connectivity button for not-installed entries", async () => {
      const wrapper = mountDetail();
      await flushPromises();

      const testBtn = document.body.querySelector('[data-test="catalog-test-connectivity"]');
      expect(testBtn).toBeNull();

      wrapper.unmount();
    });
  });

  // ── 5. Configuration preview rendering ──

  describe("configuration preview", () => {
    it("shows no-configuration message when no env vars or headers", async () => {
      const wrapper = mountDetail(
        fixtureEntry({
          install_spec_json: JSON.stringify({
            transport: "stdio",
            command: "npx",
            args: ["test"],
            env: {},
            cwd: null
          }),
          default_env_json: "[]"
        })
      );
      await flushPromises();

      expect(document.body.textContent).toContain("No configuration required.");

      wrapper.unmount();
    });

    it("renders environment variable config items", async () => {
      const wrapper = mountDetail(
        fixtureEntry({
          default_env_json: JSON.stringify([
            {
              key: "API_KEY",
              label: "API Key",
              description: "Your API key for the service.",
              required: true,
              secret: true,
              default: null
            },
            {
              key: "BASE_URL",
              label: "Base URL",
              description: "Optional base URL override.",
              required: false,
              secret: false,
              default: "https://api.example.com"
            }
          ])
        })
      );
      await flushPromises();

      const text = document.body.textContent ?? "";
      expect(text).toContain("API Key");
      expect(text).toContain("Your API key for the service.");
      expect(text).toContain("Required");
      expect(text).toContain("Base URL");
      expect(text).toContain("Optional base URL override.");
      expect(text).toContain("Optional");
      expect(text).toContain("Environment variable");

      // Secret field renders as password input
      const apiKeyInput = document.body.querySelector('[data-test="config-API_KEY"]');
      expect(apiKeyInput).not.toBeNull();
      expect(apiKeyInput?.getAttribute("type")).toBe("password");

      // Non-secret field renders as text input
      const baseUrlInput = document.body.querySelector('[data-test="config-BASE_URL"]');
      expect(baseUrlInput).not.toBeNull();
      expect(baseUrlInput?.getAttribute("type")).toBe("text");

      wrapper.unmount();
    });

    it("renders HTTP header config items for streamable_http transport", async () => {
      const wrapper = mountDetail(
        fixtureEntry({
          install_spec_json: JSON.stringify({
            transport: "streamable_http",
            url: "https://example.com/mcp",
            headers: { Authorization: "", "X-Custom": "" }
          }),
          default_env_json: JSON.stringify([
            {
              key: "Authorization",
              label: "Authorization",
              description: "Bearer token.",
              required: true,
              secret: true,
              default: null
            },
            {
              key: "X-Custom",
              label: "Custom Header",
              description: "A custom header.",
              required: false,
              secret: false,
              default: null
            }
          ])
        })
      );
      await flushPromises();

      const text = document.body.textContent ?? "";
      expect(text).toContain("Authentication header");
      expect(text).toContain("HTTP header");
      expect(text).toContain("Bearer token.");

      wrapper.unmount();
    });

    it("shows required config count summary when required items exist", async () => {
      const wrapper = mountDetail(
        fixtureEntry({
          default_env_json: JSON.stringify([
            {
              key: "TOKEN",
              label: "Token",
              description: "Auth token",
              required: true,
              secret: true,
              default: null
            }
          ])
        })
      );
      await flushPromises();

      const text = document.body.textContent ?? "";
      expect(text).toContain("Required configuration");

      wrapper.unmount();
    });

    it("shows optional config summary when all items are optional", async () => {
      const wrapper = mountDetail(
        fixtureEntry({
          default_env_json: JSON.stringify([
            {
              key: "CACHE_DIR",
              label: "Cache Directory",
              description: "Where to store cache",
              required: false,
              secret: false,
              default: "/tmp"
            }
          ])
        })
      );
      await flushPromises();

      const text = document.body.textContent ?? "";
      // Should show the optional summary, not "Required configuration"
      expect(text).not.toContain("Required configuration");

      wrapper.unmount();
    });

    it("falls back to no-description text when spec.description is empty", async () => {
      const wrapper = mountDetail(
        fixtureEntry({
          default_env_json: JSON.stringify([
            {
              key: "EMPTY_DESC",
              label: "No Desc Var",
              description: "",
              required: false,
              secret: false,
              default: null
            }
          ])
        })
      );
      await flushPromises();

      const text = document.body.textContent ?? "";
      expect(text).toContain("No description provided by the catalog.");

      wrapper.unmount();
    });
  });

  // ── 6. Empty / missing data handling ──

  describe("empty and missing data handling", () => {
    it("handles entry with null author and version gracefully", async () => {
      const wrapper = mountDetail(fixtureEntry({ author: null, version: null }));
      await flushPromises();

      // Should still render the main structure without errors
      expect(document.body.querySelector('[data-test="catalog-detail"]')).not.toBeNull();

      wrapper.unmount();
    });

    it("handles empty tags and categories", async () => {
      const wrapper = mountDetail(fixtureEntry({ tags: [], categories: [] }));
      await flushPromises();

      expect(document.body.querySelector('[data-test="catalog-detail"]')).not.toBeNull();

      wrapper.unmount();
    });

    it("handles malformed JSON in default_env_json gracefully", async () => {
      const wrapper = mountDetail(fixtureEntry({ default_env_json: "not-json" }));
      await flushPromises();

      // Should show no-configuration state (parseDefaultEnv returns [])
      expect(document.body.textContent).toContain("No configuration required.");

      wrapper.unmount();
    });

    it("handles malformed JSON in requirements_json gracefully", async () => {
      const wrapper = mountDetail(fixtureEntry({ requirements_json: "{invalid}" }));
      await flushPromises();

      // Should render without crashing
      expect(document.body.querySelector('[data-test="catalog-detail"]')).not.toBeNull();

      wrapper.unmount();
    });

    it("handles malformed JSON in install_spec_json gracefully", async () => {
      const wrapper = mountDetail(fixtureEntry({ install_spec_json: "broken" }));
      await flushPromises();

      expect(document.body.querySelector('[data-test="catalog-detail"]')).not.toBeNull();

      wrapper.unmount();
    });
  });

  // ── 7. Emitted events ──

  describe("emitted events", () => {
    it("emits close when close button is clicked", async () => {
      const wrapper = mountDetail();
      await flushPromises();

      // The KxDrawer has a close button and the footer has a close button
      // Find the close button via the wrapper emitted events
      // The drawer emits close up to the component
      expect(wrapper.emitted("close")).toBeUndefined();

      // Click the footer close button (last KxButton without variant)
      const buttons = document.body.querySelectorAll("button");
      const closeBtn = Array.from(buttons).find(
        (btn) => btn.textContent?.trim() === "Close" && !btn.hasAttribute("data-test")
      );
      expect(closeBtn).toBeTruthy();
      closeBtn?.click();
      await flushPromises();

      expect(wrapper.emitted("close")).toBeTruthy();

      wrapper.unmount();
    });
  });

  // ── 8. Options checkboxes ──

  describe("options checkboxes", () => {
    it("renders trust and auto-start checkboxes with correct defaults", async () => {
      const wrapper = mountDetail();
      await flushPromises();

      const trustCheckbox = document.body.querySelector<HTMLInputElement>(
        '[data-test="catalog-trust-checkbox"]'
      );
      const autoStartCheckbox = document.body.querySelector<HTMLInputElement>(
        '[data-test="catalog-auto-start-checkbox"]'
      );
      expect(trustCheckbox).not.toBeNull();
      expect(autoStartCheckbox).not.toBeNull();
      expect(trustCheckbox.checked).toBe(false);
      expect(autoStartCheckbox.checked).toBe(true);

      wrapper.unmount();
    });

    it("shows verified trust hint when entry trust is verified", async () => {
      const wrapper = mountDetail(fixtureEntry({ trust: "verified" }));
      await flushPromises();

      const text = document.body.textContent ?? "";
      // The component shows verifiedTrustHint text for verified entries
      expect(text).toContain("verified");

      wrapper.unmount();
    });

    it("hides verified trust hint when entry trust is not verified", async () => {
      const wrapper = mountDetail(fixtureEntry({ trust: "community" }));
      await flushPromises();

      // The hint-verified element should not be present
      const hintEl = document.body.querySelector(".hint-verified");
      expect(hintEl).toBeNull();

      wrapper.unmount();
    });
  });

  // ── 9. Scope label for installed entries ──

  describe("scope label", () => {
    it("shows User scope label for entries installed at user scope", async () => {
      seedInstalled("test-server");

      const wrapper = mountDetail();
      await flushPromises();

      const statusEl = document.body.querySelector('[data-test="catalog-installed-status"]');
      expect(statusEl).not.toBeNull();
      // effectiveServers fixture has source: "User"
      expect(statusEl?.textContent).toContain("User");

      wrapper.unmount();
    });

    it("shows Project scope when effective server source is Project", async () => {
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
      useMcpStore().effectiveServers = [
        {
          value: {
            id: "test-server",
            name: "Test Server",
            transport: "stdio",
            enabled: true,
            runtime_status: "running",
            trusted: false,
            tool_count: 2,
            last_error: null,
            writable: true,
            config_path: "/tmp/kairox.toml",
            description: "Test"
          },
          source: "Project",
          overrides: null,
          enabled: true,
          disabledBy: null,
          writable: true,
          deletable: true
        }
      ];

      const wrapper = mountDetail();
      await flushPromises();

      const statusEl = document.body.querySelector('[data-test="catalog-installed-status"]');
      expect(statusEl?.textContent).toContain("Project");

      wrapper.unmount();
    });
  });

  // ── 10. Requirements section ──

  describe("requirements section", () => {
    it("renders the requirements card", async () => {
      const wrapper = mountDetail(
        fixtureEntry({
          requirements_json: JSON.stringify([
            { kind: "node", min_version: ">=18.0.0", install_hint: "https://nodejs.org" }
          ])
        })
      );
      await flushPromises();

      const text = document.body.textContent ?? "";
      expect(text).toContain("Requirements");

      wrapper.unmount();
    });
  });
});

import { describe, it, expect, beforeEach, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { ref } from "vue";
import PluginSettingsPane from "./PluginSettingsPane.vue";
import pluginSettingsPaneSource from "./PluginSettingsPane.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

vi.mock("@/generated/commands", () => ({
  commands: {
    listPluginSettings: vi.fn(),
    listPluginMarketplaceSources: vi.fn(),
    listPluginCatalog: vi.fn(),
    setPluginMarketplaceSourceEnabled: vi.fn(),
    setPluginEnabled: vi.fn(),
    deletePluginSettings: vi.fn(),
    installPlugin: vi.fn()
  }
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { commands } from "@/generated/commands";
const mockedCommands = vi.mocked(commands);

function ok<T>(data: T): { status: "ok"; data: T } {
  return { status: "ok", data };
}

function err(error: string): { status: "error"; error: string } {
  return { status: "error", error };
}

function mountPane(configSource: "user" | "project" = "user") {
  return mountWithPlugins(PluginSettingsPane, {
    reusePinia: true,
    mount: {
      global: {
        provide: { configSource: ref(configSource) }
      }
    }
  });
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("PluginSettingsPane", () => {
  describe("tab structure", () => {
    it("uses a single marketplace tab with source settings inside it", async () => {
      mockedCommands.listPluginSettings.mockResolvedValue(ok([]));
      mockedCommands.listPluginMarketplaceSources.mockResolvedValue(ok([]));
      mockedCommands.listPluginCatalog.mockResolvedValue(ok([]));

      const { wrapper } = mountPane();
      await flushPromises();

      expect(wrapper.find('[data-test="plugin-subtab-installed"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="plugin-subtab-marketplace"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="plugin-subtab-discover"]').exists()).toBe(false);
      expect(wrapper.find('[data-test="plugin-subtab-marketplaces"]').exists()).toBe(false);

      await wrapper.find('[data-test="plugin-subtab-marketplace"]').trigger("click");
      await flushPromises();

      expect(wrapper.find('[data-test="plugin-source-settings-toggle"]').exists()).toBe(true);
    });
  });

  describe("installed plugins", () => {
    it("renders installed plugins with name, scope, and status", async () => {
      mockedCommands.listPluginSettings.mockResolvedValue(
        ok([
          {
            settings_id: "User:github",
            id: "github",
            name: "GitHub",
            description: "Browse and manage GitHub repositories.",
            version: "1.0.0",
            scope: "User",
            path: "/Users/mock/.config/kairox/plugins/github",
            enabled: true,
            install_source: "marketplace",
            marketplace: "anthropics-claude-code",
            effective: true,
            shadowed_by: null,
            valid: true,
            validation_error: null,
            inventory: {
              skill_count: 0,
              skill_names: [],
              mcp_server_count: 1,
              app_count: 0,
              agent_count: 0,
              hook_count: 0
            },
            manifest_kind: "claude"
          }
        ])
      );
      mockedCommands.listPluginMarketplaceSources.mockResolvedValue(ok([]));

      const { wrapper } = mountPane();
      await flushPromises();

      const row = wrapper.find('[data-test="plugin-row-user-github"]');
      expect(row.exists()).toBe(true);
      expect(row.classes()).toContain("settings-card-item");
      expect(wrapper.find('[data-test="plugin-installed-list"]').classes()).toContain(
        "settings-card-list"
      );
      expect(row.text()).toContain("GitHub");
      expect(row.text()).toContain("User");
    });

    it("uses shared card content hierarchy instead of plugin-local title and meta css", () => {
      expectSourceMigration(pluginSettingsPaneSource, {
        required: ["SettingsItemSummary", "SettingsItemMeta", "SettingsStatusTag"],
        forbidden: [".plugin-row__title", ".plugin-meta", "tag-success", "tag-warning", "tag-error"]
      });
    });

    it("shows empty state when no plugins installed", async () => {
      mockedCommands.listPluginSettings.mockResolvedValue(ok([]));
      mockedCommands.listPluginMarketplaceSources.mockResolvedValue(ok([]));

      const { wrapper } = mountPane();
      await flushPromises();

      expect(wrapper.find('[data-test="plugin-row-user-github"]').exists()).toBe(false);
      const empty = wrapper.find('[data-test="plugin-empty-state"]');
      expect(empty.classes()).toContain("settings-state");
      expect(empty.text()).toContain("No plugins installed");
    });
  });

  describe("toggle plugin", () => {
    it("disables an enabled plugin and reloads", async () => {
      const plugin = {
        settings_id: "User:github",
        id: "github",
        name: "GitHub",
        description: "Browse and manage GitHub repositories.",
        version: "1.0.0",
        scope: "User" as const,
        path: "/Users/mock/.config/kairox/plugins/github",
        enabled: true,
        install_source: "marketplace",
        marketplace: "anthropics-claude-code",
        effective: true,
        shadowed_by: null,
        valid: true,
        validation_error: null,
        inventory: {
          skill_count: 0,
          skill_names: [],
          mcp_server_count: 1,
          app_count: 0,
          agent_count: 0,
          hook_count: 0
        },
        manifest_kind: "claude"
      };
      const disabledPlugin = { ...plugin, enabled: false };

      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([plugin]));
      mockedCommands.listPluginMarketplaceSources.mockResolvedValueOnce(ok([]));
      mockedCommands.setPluginEnabled.mockResolvedValueOnce(ok(null));
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([disabledPlugin]));

      const { wrapper } = mountPane();
      await flushPromises();

      const toggleBtn = wrapper.find('[data-test="plugin-enabled-user-github"]');
      expect(toggleBtn.exists()).toBe(true);

      await toggleBtn.trigger("click");
      await flushPromises();

      expect(mockedCommands.setPluginEnabled).toHaveBeenCalledWith("User:github", false);
      expect(wrapper.find('[data-test="plugin-row-user-github"]').text()).toContain("Disabled");
    });
  });

  describe("delete plugin", () => {
    it("deletes a plugin and reloads list", async () => {
      const plugin = {
        settings_id: "User:quality-review",
        id: "quality-review",
        name: "Quality Review",
        description: "Review code quality.",
        version: "0.1.0",
        scope: "User" as const,
        path: "/Users/mock/.config/kairox/plugins/quality-review",
        enabled: true,
        install_source: "marketplace",
        marketplace: "anthropics-claude-code",
        effective: true,
        shadowed_by: null,
        valid: true,
        validation_error: null,
        inventory: {
          skill_count: 1,
          skill_names: ["quality-review"],
          mcp_server_count: 0,
          app_count: 0,
          agent_count: 0,
          hook_count: 0
        },
        manifest_kind: "claude"
      };

      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([plugin]));
      mockedCommands.listPluginMarketplaceSources.mockResolvedValueOnce(ok([]));
      mockedCommands.deletePluginSettings.mockResolvedValueOnce(ok(null));
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([]));

      const { wrapper } = mountPane();
      await flushPromises();

      const deleteBtn = wrapper.find('[data-test="plugin-delete-user-quality-review"]');
      expect(deleteBtn.exists()).toBe(true);

      await deleteBtn.trigger("click");
      await flushPromises();

      expect(mockedCommands.deletePluginSettings).toHaveBeenCalledWith("User:quality-review");
      expect(wrapper.find('[data-test="plugin-row-user-quality-review"]').exists()).toBe(false);
    });
  });

  describe("marketplace catalog", () => {
    it("shows catalog entries after switching to marketplace tab", async () => {
      mockedCommands.listPluginSettings.mockResolvedValue(ok([]));
      mockedCommands.listPluginMarketplaceSources.mockResolvedValue(
        ok([
          {
            id: "anthropics-claude-code",
            display_name: "Anthropic Claude Code Demo",
            source: "https://anthropic.github.io/claude-code-registry/index.json",
            enabled: true,
            builtin: true
          }
        ])
      );
      mockedCommands.listPluginCatalog.mockResolvedValue(
        ok([
          {
            marketplace_id: "anthropics-claude-code",
            name: "quality-review",
            description: "Review code quality and suggest improvements.",
            version: "0.1.0",
            source: "https://github.com/anthropics/claude-code-registry/plugins/quality-review"
          }
        ])
      );

      const { wrapper } = mountPane();
      await flushPromises();

      await wrapper.find('[data-test="plugin-subtab-marketplace"]').trigger("click");
      await flushPromises();

      const card = wrapper.find('[data-test="plugin-catalog-card"]');
      expect(card.exists()).toBe(true);
      expect(card.text()).toContain("quality-review");
    });
  });

  describe("install from catalog", () => {
    it("installs a catalog entry and shows it in installed list", async () => {
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([]));
      mockedCommands.listPluginMarketplaceSources.mockResolvedValueOnce(
        ok([
          {
            id: "anthropics-claude-code",
            display_name: "Anthropic Claude Code Demo",
            source: "https://anthropic.github.io/claude-code-registry/index.json",
            enabled: true,
            builtin: true
          }
        ])
      );
      mockedCommands.listPluginCatalog.mockResolvedValueOnce(
        ok([
          {
            marketplace_id: "anthropics-claude-code",
            name: "quality-review",
            description: "Review code quality and suggest improvements.",
            version: "0.1.0",
            source: "https://github.com/anthropics/claude-code-registry/plugins/quality-review"
          }
        ])
      );

      const installed = {
        settings_id: "User:quality-review",
        id: "quality-review",
        name: "Quality Review",
        description: "Review code quality and suggest improvements.",
        version: "0.1.0",
        scope: "User" as const,
        path: "/Users/mock/.config/kairox/plugins/quality-review",
        enabled: true,
        install_source: "marketplace",
        marketplace: "anthropics-claude-code",
        effective: true,
        shadowed_by: null,
        valid: true,
        validation_error: null,
        inventory: {
          skill_count: 1,
          skill_names: ["quality-review"],
          mcp_server_count: 0,
          app_count: 0,
          agent_count: 0,
          hook_count: 0
        },
        manifest_kind: "claude"
      };

      mockedCommands.installPlugin.mockResolvedValueOnce(ok(installed));
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([installed]));

      const { wrapper } = mountPane();
      await flushPromises();

      await wrapper.find('[data-test="plugin-subtab-marketplace"]').trigger("click");
      await flushPromises();

      const installBtn = wrapper.find(
        '[data-test="plugin-install-anthropics-claude-code-quality-review"]'
      );
      expect(installBtn.exists()).toBe(true);

      await installBtn.trigger("click");
      await flushPromises();

      expect(mockedCommands.installPlugin).toHaveBeenCalledWith({
        marketplace_id: "anthropics-claude-code",
        plugin_name: "quality-review",
        target: "user"
      });

      await wrapper.find('[data-test="plugin-subtab-installed"]').trigger("click");
      await flushPromises();

      expect(wrapper.find('[data-test="plugin-row-user-quality-review"]').exists()).toBe(true);
    });
  });

  describe("source settings", () => {
    it("toggles a marketplace source enabled state", async () => {
      mockedCommands.listPluginSettings.mockResolvedValue(ok([]));
      mockedCommands.listPluginMarketplaceSources.mockResolvedValue(
        ok([
          {
            id: "anthropics-claude-code",
            display_name: "Anthropic Claude Code Demo",
            source: "https://anthropic.github.io/claude-code-registry/index.json",
            enabled: true,
            builtin: true
          }
        ])
      );
      mockedCommands.listPluginCatalog.mockResolvedValue(ok([]));
      mockedCommands.setPluginMarketplaceSourceEnabled.mockResolvedValueOnce(ok(null));
      mockedCommands.listPluginMarketplaceSources.mockResolvedValueOnce(
        ok([
          {
            id: "anthropics-claude-code",
            display_name: "Anthropic Claude Code Demo",
            source: "https://anthropic.github.io/claude-code-registry/index.json",
            enabled: false,
            builtin: true
          }
        ])
      );
      mockedCommands.listPluginCatalog.mockResolvedValueOnce(ok([]));

      const { wrapper } = mountPane();
      await flushPromises();

      await wrapper.find('[data-test="plugin-subtab-marketplace"]').trigger("click");
      await flushPromises();

      await wrapper.find('[data-test="plugin-source-settings-toggle"]').trigger("click");
      await flushPromises();

      const sourceToggle = wrapper.find(
        '[data-test="plugin-source-enabled-anthropics-claude-code"]'
      );
      expect(sourceToggle.exists()).toBe(true);

      await sourceToggle.trigger("click");
      await flushPromises();

      expect(mockedCommands.setPluginMarketplaceSourceEnabled).toHaveBeenCalledWith(
        "anthropics-claude-code",
        false
      );
    });
  });

  describe("error display", () => {
    it("renders error banner when store has error", async () => {
      mockedCommands.listPluginSettings.mockResolvedValue(err("plugins unavailable"));
      mockedCommands.listPluginMarketplaceSources.mockResolvedValue(ok([]));

      const { wrapper } = mountPane();
      await flushPromises();

      const errorBanner = wrapper.find('[data-test="plugin-error"]');
      expect(errorBanner.exists()).toBe(true);
      expect(errorBanner.classes()).toContain("settings-state");
      expect(errorBanner.text()).toContain("plugins unavailable");
    });
  });

  describe("shared settings card primitives", () => {
    it("does not keep local plugin row chrome after moving to SettingsCardItem", () => {
      expectSourceMigration(pluginSettingsPaneSource, {
        required: ["SettingsCardList", "SettingsCardItem"],
        forbidden: [".plugin-row {"]
      });
    });

    it("uses shared settings toolbar, subtabs, and filter bar instead of local plugin chrome", () => {
      expectSourceMigration(pluginSettingsPaneSource, {
        required: ["SettingsSubtabs", "SettingsToolbar", "SettingsFilterBar"],
        forbidden: [
          'class="plugin-sub-tabs"',
          'class="plugin-toolbar"',
          ".plugin-sub-tabs,",
          ".plugin-toolbar {",
          ".sub-tab-btn {"
        ]
      });
    });
  });
});

import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { usePluginsStore } from "@/stores/plugins";
import type {
  PluginSettingsView,
  PluginMarketplaceSourceView,
  PluginCatalogEntry
} from "@/generated/commands";

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

import { commands } from "@/generated/commands";
const mockedCommands = vi.mocked(commands);

function ok<T>(data: T): { status: "ok"; data: T } {
  return { status: "ok", data };
}

function err(error: string): { status: "error"; error: string } {
  return { status: "error", error };
}

function createPluginSetting(overrides: Partial<PluginSettingsView> = {}): PluginSettingsView {
  const id = overrides.id ?? "github";
  const scope = overrides.scope ?? "User";
  return {
    settings_id: `${scope}:${id}`,
    id,
    name: id.charAt(0).toUpperCase() + id.slice(1),
    description: "Browse and manage GitHub repositories.",
    version: "1.0.0",
    scope,
    path: `/Users/mock/.config/kairox/plugins/${id}`,
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
    manifest_kind: "claude",
    ...overrides
  };
}

function createSourceView(
  overrides: Partial<PluginMarketplaceSourceView> = {}
): PluginMarketplaceSourceView {
  return {
    id: "anthropics-claude-code",
    display_name: "Anthropic Claude Code Demo",
    source: "https://anthropic.github.io/claude-code-registry/index.json",
    enabled: true,
    builtin: true,
    ...overrides
  };
}

function createCatalogEntry(overrides: Partial<PluginCatalogEntry> = {}): PluginCatalogEntry {
  return {
    marketplace_id: "anthropics-claude-code",
    name: "quality-review",
    description: "Review code quality and suggest improvements.",
    version: "0.1.0",
    source: "https://github.com/anthropics/claude-code-registry/plugins/quality-review",
    ...overrides
  };
}

describe("plugins store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  describe("loadPlugins", () => {
    it("loads installed plugins and exposes effective subset", async () => {
      const userPlugin = createPluginSetting({ settings_id: "User:github", scope: "User" });
      const projectPlugin = createPluginSetting({
        settings_id: "Project:github",
        scope: "Project",
        path: "/mock/workspace/.kairox/plugins/github"
      });
      const shadowedPlugin = createPluginSetting({
        settings_id: "Project:slack",
        scope: "Project",
        id: "slack",
        name: "Slack",
        effective: false,
        shadowed_by: "User"
      });
      mockedCommands.listPluginSettings.mockResolvedValueOnce(
        ok([userPlugin, projectPlugin, shadowedPlugin])
      );

      const store = usePluginsStore();
      await store.loadPlugins();

      expect(store.plugins).toHaveLength(3);
      expect(store.effectivePlugins).toHaveLength(2);
      expect(store.effectivePlugins.map((p) => p.settings_id)).toEqual([
        "User:github",
        "Project:github"
      ]);
      expect(store.loading).toBe(false);
      expect(store.error).toBeNull();
    });

    it("returns empty plugins on first load", async () => {
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([]));

      const store = usePluginsStore();
      await store.loadPlugins();

      expect(store.plugins).toHaveLength(0);
      expect(store.effectivePlugins).toHaveLength(0);
    });

    it("sets error and clears loading on failure", async () => {
      mockedCommands.listPluginSettings.mockResolvedValueOnce(err("plugins unavailable"));

      const store = usePluginsStore();
      await store.loadPlugins();

      expect(store.error).toContain("plugins unavailable");
      expect(store.loading).toBe(false);
      expect(store.plugins).toHaveLength(0);
    });
  });

  describe("loadSources", () => {
    it("loads marketplace sources", async () => {
      const builtinSource = createSourceView();
      const customSource = createSourceView({
        id: "custom-registry",
        display_name: "Custom Registry",
        builtin: false
      });
      mockedCommands.listPluginMarketplaceSources.mockResolvedValueOnce(
        ok([builtinSource, customSource])
      );

      const store = usePluginsStore();
      await store.loadSources();

      expect(store.sources).toHaveLength(2);
      expect(store.sources[0].id).toBe("anthropics-claude-code");
      expect(store.sources[1].builtin).toBe(false);
    });

    it("sets error on source load failure", async () => {
      mockedCommands.listPluginMarketplaceSources.mockResolvedValueOnce(err("network unavailable"));

      const store = usePluginsStore();
      await store.loadSources();

      expect(store.error).toContain("network unavailable");
    });
  });

  describe("loadCatalog", () => {
    it("loads catalog entries", async () => {
      const entry = createCatalogEntry();
      mockedCommands.listPluginCatalog.mockResolvedValueOnce(ok([entry]));

      const store = usePluginsStore();
      await store.loadCatalog("anthropics-claude-code", "quality");

      expect(mockedCommands.listPluginCatalog).toHaveBeenCalledWith(
        "anthropics-claude-code",
        "quality"
      );
      expect(store.catalog).toHaveLength(1);
      expect(store.catalog[0].name).toBe("quality-review");
      expect(store.catalogLoading).toBe(false);
    });

    it("passes null marketplace and keyword for full catalog", async () => {
      mockedCommands.listPluginCatalog.mockResolvedValueOnce(ok([]));

      const store = usePluginsStore();
      await store.loadCatalog(null, "");

      expect(mockedCommands.listPluginCatalog).toHaveBeenCalledWith(null, "");
    });

    it("sets error and clears catalogLoading on failure", async () => {
      mockedCommands.listPluginCatalog.mockResolvedValueOnce(err("catalog unavailable"));

      const store = usePluginsStore();
      await store.loadCatalog(null, null);

      expect(store.error).toContain("catalog unavailable");
      expect(store.catalogLoading).toBe(false);
    });
  });

  describe("setPluginEnabled", () => {
    it("toggles plugin from enabled to disabled", async () => {
      mockedCommands.setPluginEnabled.mockResolvedValueOnce(ok(null));
      mockedCommands.listPluginSettings.mockResolvedValueOnce(
        ok([createPluginSetting({ enabled: false })])
      );

      const store = usePluginsStore();
      store.plugins = [createPluginSetting({ enabled: true })];

      await store.setPluginEnabled("User:github", false);

      expect(mockedCommands.setPluginEnabled).toHaveBeenCalledWith("User:github", false);
      expect(store.plugins[0].enabled).toBe(false);
    });

    it("toggles plugin from disabled to enabled", async () => {
      mockedCommands.setPluginEnabled.mockResolvedValueOnce(ok(null));
      mockedCommands.listPluginSettings.mockResolvedValueOnce(
        ok([createPluginSetting({ enabled: true })])
      );

      const store = usePluginsStore();
      store.plugins = [createPluginSetting({ enabled: false })];

      await store.setPluginEnabled("User:github", true);

      expect(mockedCommands.setPluginEnabled).toHaveBeenCalledWith("User:github", true);
      expect(store.plugins[0].enabled).toBe(true);
    });

    it("sets busyPluginId during toggle operation", async () => {
      mockedCommands.setPluginEnabled.mockResolvedValueOnce(ok(null));
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([]));

      const store = usePluginsStore();
      const promise = store.setPluginEnabled("User:github", false);

      expect(store.busyPluginId).toBe("User:github");

      await promise;

      expect(store.busyPluginId).toBeNull();
    });

    it("clears busyPluginId even on failure", async () => {
      mockedCommands.setPluginEnabled.mockResolvedValueOnce(err("permission denied"));

      const store = usePluginsStore();
      await store.setPluginEnabled("User:github", false);

      expect(store.busyPluginId).toBeNull();
      expect(store.error).toContain("permission denied");
    });

    it("does not update plugin list on toggle failure", async () => {
      mockedCommands.setPluginEnabled.mockResolvedValueOnce(err("state file is read-only"));

      const store = usePluginsStore();
      store.plugins = [createPluginSetting({ enabled: true })];

      await store.setPluginEnabled("User:github", false);

      expect(mockedCommands.listPluginSettings).not.toHaveBeenCalled();
      expect(store.plugins[0].enabled).toBe(true);
    });
  });

  describe("deletePlugin", () => {
    it("removes plugin and reloads list", async () => {
      mockedCommands.deletePluginSettings.mockResolvedValueOnce(ok(null));
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([]));

      const store = usePluginsStore();
      store.plugins = [createPluginSetting()];

      await store.deletePlugin("User:github");

      expect(mockedCommands.deletePluginSettings).toHaveBeenCalledWith("User:github");
      expect(store.plugins).toHaveLength(0);
    });

    it("sets busyPluginId during delete operation", async () => {
      mockedCommands.deletePluginSettings.mockResolvedValueOnce(ok(null));
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([]));

      const store = usePluginsStore();
      const promise = store.deletePlugin("User:github");

      expect(store.busyPluginId).toBe("User:github");

      await promise;

      expect(store.busyPluginId).toBeNull();
    });

    it("sets error and clears busyPluginId on delete failure", async () => {
      mockedCommands.deletePluginSettings.mockResolvedValueOnce(err("cannot delete builtin"));

      const store = usePluginsStore();
      await store.deletePlugin("User:github");

      expect(store.error).toContain("cannot delete builtin");
      expect(store.busyPluginId).toBeNull();
    });
  });

  describe("installPlugin", () => {
    it("installs a plugin into user scope and reloads", async () => {
      const installed = createPluginSetting({
        settings_id: "User:quality-review",
        id: "quality-review",
        name: "Quality Review",
        scope: "User"
      });
      mockedCommands.installPlugin.mockResolvedValueOnce(ok(installed));
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([installed]));

      const store = usePluginsStore();
      const result = await store.installPlugin("anthropics-claude-code", "quality-review", "user");

      expect(mockedCommands.installPlugin).toHaveBeenCalledWith({
        marketplace_id: "anthropics-claude-code",
        plugin_name: "quality-review",
        target: "user"
      });
      expect(result).toEqual(installed);
      expect(store.plugins).toHaveLength(1);
    });

    it("installs a plugin into project scope", async () => {
      const installed = createPluginSetting({
        settings_id: "Project:quality-review",
        id: "quality-review",
        name: "Quality Review",
        scope: "Project",
        path: "/mock/workspace/.kairox/plugins/quality-review"
      });
      mockedCommands.installPlugin.mockResolvedValueOnce(ok(installed));
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([installed]));

      const store = usePluginsStore();
      const result = await store.installPlugin(
        "anthropics-claude-code",
        "quality-review",
        "project"
      );

      expect(mockedCommands.installPlugin).toHaveBeenCalledWith({
        marketplace_id: "anthropics-claude-code",
        plugin_name: "quality-review",
        target: "project"
      });
      expect(result?.scope).toBe("Project");
    });

    it("sets busyPluginId during install", async () => {
      mockedCommands.installPlugin.mockResolvedValueOnce(ok(createPluginSetting()));
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([]));

      const store = usePluginsStore();
      const promise = store.installPlugin("anthropics-claude-code", "quality-review", "user");

      expect(store.busyPluginId).toBe("anthropics-claude-code:quality-review");

      await promise;

      expect(store.busyPluginId).toBeNull();
    });

    it("returns null and sets error on install failure", async () => {
      mockedCommands.installPlugin.mockResolvedValueOnce(err("manifest not found"));

      const store = usePluginsStore();
      const result = await store.installPlugin("anthropics-claude-code", "quality-review", "user");

      expect(result).toBeNull();
      expect(store.error).toContain("manifest not found");
      expect(store.busyPluginId).toBeNull();
    });

    it("does not reload plugins on install failure", async () => {
      mockedCommands.installPlugin.mockResolvedValueOnce(err("network error"));

      const store = usePluginsStore();
      await store.installPlugin("anthropics-claude-code", "quality-review", "user");

      expect(mockedCommands.listPluginSettings).not.toHaveBeenCalled();
    });
  });

  describe("setMarketplaceSourceEnabled", () => {
    it("enables a marketplace source and refreshes catalog", async () => {
      const source = createSourceView({ enabled: false });
      const enabledSource = createSourceView({ enabled: true });
      mockedCommands.setPluginMarketplaceSourceEnabled.mockResolvedValueOnce(ok(null));
      mockedCommands.listPluginMarketplaceSources.mockResolvedValueOnce(ok([enabledSource]));
      mockedCommands.listPluginCatalog.mockResolvedValueOnce(ok([createCatalogEntry()]));

      const store = usePluginsStore();
      store.sources = [source];

      await store.setMarketplaceSourceEnabled("anthropics-claude-code", true);

      expect(mockedCommands.setPluginMarketplaceSourceEnabled).toHaveBeenCalledWith(
        "anthropics-claude-code",
        true
      );
      expect(store.sources[0].enabled).toBe(true);
      expect(store.catalog).toHaveLength(1);
    });

    it("disables a marketplace source", async () => {
      const disabledSource = createSourceView({ enabled: false });
      mockedCommands.setPluginMarketplaceSourceEnabled.mockResolvedValueOnce(ok(null));
      mockedCommands.listPluginMarketplaceSources.mockResolvedValueOnce(ok([disabledSource]));
      mockedCommands.listPluginCatalog.mockResolvedValueOnce(ok([]));

      const store = usePluginsStore();
      store.sources = [createSourceView({ enabled: true })];

      await store.setMarketplaceSourceEnabled("anthropics-claude-code", false);

      expect(mockedCommands.setPluginMarketplaceSourceEnabled).toHaveBeenCalledWith(
        "anthropics-claude-code",
        false
      );
      expect(store.sources[0].enabled).toBe(false);
    });

    it("sets error on source toggle failure", async () => {
      mockedCommands.setPluginMarketplaceSourceEnabled.mockResolvedValueOnce(
        err("source is builtin and cannot be disabled")
      );

      const store = usePluginsStore();
      await store.setMarketplaceSourceEnabled("anthropics-claude-code", false);

      expect(store.error).toContain("source is builtin and cannot be disabled");
    });
  });

  describe("computed", () => {
    it("effectivePlugins filters out shadowed plugins", () => {
      const store = usePluginsStore();
      store.plugins = [
        createPluginSetting({ settings_id: "User:github", effective: true }),
        createPluginSetting({
          settings_id: "Project:slack",
          id: "slack",
          name: "Slack",
          scope: "Project",
          effective: false,
          shadowed_by: "User"
        }),
        createPluginSetting({
          settings_id: "User:invalid",
          id: "invalid-plugin",
          name: "Invalid",
          valid: false,
          validation_error: "missing manifest"
        })
      ];

      expect(store.effectivePlugins).toHaveLength(2);
      expect(store.effectivePlugins[0].settings_id).toBe("User:github");
      expect(store.effectivePlugins[1].settings_id).toBe("User:invalid");
    });
  });

  describe("error clearing", () => {
    it("clears previous error on successful operation", async () => {
      mockedCommands.listPluginSettings.mockResolvedValueOnce(ok([]));

      const store = usePluginsStore();
      store.error = "previous error";

      await store.loadPlugins();

      expect(store.error).toBeNull();
    });
  });
});

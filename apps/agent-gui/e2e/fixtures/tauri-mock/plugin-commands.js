/**
 * Browser-side Tauri mock fragment for plugin settings commands.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

function findPluginSetting(pluginId) {
  var settingsIdMatches = state.pluginSettings.filter(function (plugin) {
    return plugin.settings_id === pluginId;
  });
  if (settingsIdMatches.length === 1) return settingsIdMatches[0];
  if (settingsIdMatches.length > 1) throw new Error("ambiguous plugin settings id: " + pluginId);

  var legacyIdMatches = state.pluginSettings.filter(function (plugin) {
    return plugin.id === pluginId;
  });
  if (legacyIdMatches.length === 1) return legacyIdMatches[0];
  if (legacyIdMatches.length > 1) throw new Error("ambiguous plugin id: " + pluginId);
  return null;
}

function createPluginSettingFromCatalog(entry, target) {
  var scope = target === "project" ? "Project" : "User";
  return {
    settings_id: scope + ":" + entry.name,
    id: entry.name,
    name: entry.name
      .split("-")
      .map(function (part) {
        return part.charAt(0).toUpperCase() + part.slice(1);
      })
      .join(" "),
    description: entry.description,
    version: entry.version,
    scope: scope,
    path:
      scope === "Project"
        ? "/mock/workspace/.kairox/plugins/" + entry.name
        : "/Users/mock/.config/kairox/plugins/" + entry.name,
    enabled: true,
    install_source: "marketplace",
    marketplace: entry.marketplace_id,
    effective: true,
    shadowed_by: null,
    valid: true,
    validation_error: null,
    inventory: {
      skill_count: 1,
      skill_names: [entry.name],
      mcp_server_count: 0,
      app_count: 0,
      agent_count: 0,
      hook_count: 0
    },
    manifest_kind: "claude"
  };
}

registerCommandHandlers({
  list_plugin_settings: function () {
    return clone(state.pluginSettings);
  },
  get_plugin_detail: function (args) {
    var plugin = findPluginSetting(args.settingsId);
    if (!plugin) return Promise.reject(new Error("Plugin not found: " + args.settingsId));
    return {
      view: clone(plugin),
      manifest_path: plugin.path + "/.kairox-plugin/plugin.json",
      homepage: null,
      repository: null,
      license: null,
      keywords: []
    };
  },
  set_plugin_enabled: function (args) {
    var plugin = findPluginSetting(args.settingsId);
    if (!plugin) return Promise.reject(new Error("Plugin not found: " + args.settingsId));
    plugin.enabled = args.enabled;
    return null;
  },
  delete_plugin_settings: function (args) {
    var plugin = findPluginSetting(args.settingsId);
    if (!plugin) return Promise.reject(new Error("Plugin not found: " + args.settingsId));
    state.pluginSettings = state.pluginSettings.filter(function (candidate) {
      return candidate.settings_id !== plugin.settings_id;
    });
    return null;
  },
  list_plugin_marketplace_sources: function () {
    return clone(state.pluginMarketplaceSources);
  },
  set_plugin_marketplace_source_enabled: function (args) {
    const source = state.pluginMarketplaceSources.find((item) => item.id === args.sourceId);
    if (source) source.enabled = args.enabled;
    return null;
  },
  list_plugin_catalog: function (args) {
    var entries = state.pluginCatalog;
    if (args.marketplaceId) {
      entries = entries.filter(function (entry) {
        return entry.marketplace_id === args.marketplaceId;
      });
    }
    if (args.keyword) {
      var keyword = String(args.keyword).toLowerCase();
      entries = entries.filter(function (entry) {
        return (
          entry.name.toLowerCase().indexOf(keyword) !== -1 ||
          entry.description.toLowerCase().indexOf(keyword) !== -1
        );
      });
    }
    return clone(entries);
  },
  install_plugin: function (args) {
    var request = args.request;
    var entry = state.pluginCatalog.find(function (candidate) {
      return (
        candidate.marketplace_id === request.marketplace_id &&
        candidate.name === request.plugin_name
      );
    });
    if (!entry) return Promise.reject(new Error("Plugin not found: " + request.plugin_name));
    var plugin = createPluginSettingFromCatalog(entry, request.target);
    state.pluginSettings = state.pluginSettings.filter(function (candidate) {
      return candidate.settings_id !== plugin.settings_id;
    });
    state.pluginSettings.push(plugin);
    return clone(plugin);
  }
});

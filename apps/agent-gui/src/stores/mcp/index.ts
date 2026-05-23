// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Pinia stores are plain `.ts` modules and
// must import `defineStore`, `ref`, and `computed` explicitly.
import { defineStore } from "pinia";

import { createMcpState } from "./state";
import { createTools } from "./tools";
import { createSettings } from "./settings";
import { createConnectivity } from "./connectivity";
import { createResources } from "./resources";
import { createLifecycle } from "./lifecycle";
import { createHealth } from "./health";

export type { McpServerEntry } from "./types";

export const useMcpStore = defineStore("mcp", () => {
  const state = createMcpState();

  const tools = createTools(state);
  const settings = createSettings(state);
  const connectivity = createConnectivity(state);
  const resources = createResources(state);

  const lifecycle = createLifecycle(state, {
    updateToolCount: settings.updateToolCount,
    loadDisabledTools: tools.loadDisabledTools
  });

  const health = createHealth(state, {
    updateToolCount: settings.updateToolCount,
    loadDisabledTools: tools.loadDisabledTools,
    refreshTools: lifecycle.refreshTools,
    fetchSettingsServers: settings.fetchSettingsServers,
    fetchEffectiveServers: settings.fetchEffectiveServers
  });

  return {
    // ── State (preserves original public surface order) ──
    servers: state.servers,
    trustedServerIds: state.trustedServerIds,
    loading: state.loading,
    settingsServers: state.settingsServers,
    settingsLoading: state.settingsLoading,
    configFileOpening: state.configFileOpening,
    settingsError: state.settingsError,
    runningServers: state.runningServers,
    failedServers: state.failedServers,
    runningCount: state.runningCount,
    hasServers: state.hasServers,

    // ── Lifecycle actions ──
    fetchServers: lifecycle.fetchServers,
    startServer: lifecycle.startServer,
    stopServer: lifecycle.stopServer,
    trustServer: lifecycle.trustServer,
    revokeTrust: lifecycle.revokeTrust,
    refreshTools: lifecycle.refreshTools,

    // ── Settings actions ──
    fetchSettingsServers: settings.fetchSettingsServers,
    refreshInstalledServers: health.refreshInstalledServers,
    saveServerSettings: settings.saveServerSettings,
    setServerEnabled: settings.setServerEnabled,
    deleteServerSettings: settings.deleteServerSettings,
    disableServerAtScope: settings.disableServerAtScope,
    enableServerAtScope: settings.enableServerAtScope,
    openConfigFile: settings.openConfigFile,
    effectiveServers: state.effectiveServers,
    fetchEffectiveServers: settings.fetchEffectiveServers,

    // ── Connectivity ──
    connectivityResults: state.connectivityResults,
    testingConnectivity: state.testingConnectivity,
    testConnectivity: connectivity.testConnectivity,
    testAllConnectivity: connectivity.testAllConnectivity,

    // ── Event bridge ──
    handleMcpEvent: lifecycle.handleMcpEvent,

    // ── Health check + tool management ──
    serverHealth: state.serverHealth,
    checkingHealth: state.checkingHealth,
    expandedServers: state.expandedServers,
    disabledTools: state.disabledTools,
    checkHealth: health.checkHealth,
    checkAllHealth: health.checkAllHealth,
    isToolDisabled: tools.isToolDisabled,
    setToolDisabled: tools.setToolDisabled,
    toggleExpanded: tools.toggleExpanded,

    // ── Resource & prompt browsing ──
    serverResources: state.serverResources,
    serverPrompts: state.serverPrompts,
    loadingResources: state.loadingResources,
    loadingPrompts: state.loadingPrompts,
    expandedResourceUri: state.expandedResourceUri,
    resourcesError: state.resourcesError,
    promptsError: state.promptsError,
    resourceContentCache: state.resourceContentCache,
    fetchResources: resources.fetchResources,
    fetchPrompts: resources.fetchPrompts,
    readResource: resources.readResource,
    toggleResourceExpand: resources.toggleResourceExpand
  };
});

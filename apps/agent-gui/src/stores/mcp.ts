// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Pinia stores are plain `.ts` modules and
// must import `defineStore`, `ref`, and `computed` explicitly.
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  commands,
  type CheckMcpHealthResponse,
  type ConnectivityResult,
  type EffectiveMcpServerView,
  type McpServerSettingsInput,
  type McpServerSettingsView,
  type McpServerStatusResponse,
  type McpToolDefResponse
} from "@/generated/commands";
import { useUiStore } from "@/stores/ui";

export interface McpServerEntry extends McpServerStatusResponse {
  error?: string;
}

type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: string };
type RefreshInstalledOptions = { forceTools?: boolean };

function formatError(caughtError: unknown): string {
  return caughtError instanceof Error ? caughtError.message : String(caughtError);
}

function isCommandResult<T>(result: T | CommandResult<T>): result is CommandResult<T> {
  return (
    typeof result === "object" &&
    result !== null &&
    "status" in result &&
    (result.status === "ok" || result.status === "error")
  );
}

async function unwrapCommandResult<T>(resultPromise: Promise<T | CommandResult<T>>): Promise<T> {
  const result = await resultPromise;
  if (!isCommandResult(result)) {
    return result;
  }
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

export const useMcpStore = defineStore("mcp", () => {
  const servers = ref<McpServerEntry[]>([]);
  const trustedServerIds = ref<string[]>([]);
  const loading = ref(false);
  const settingsServers = ref<McpServerSettingsView[]>([]);
  const settingsLoading = ref(false);
  const configFileOpening = ref(false);
  const settingsError = ref<string | null>(null);
  const effectiveServers = ref<EffectiveMcpServerView[]>([]);
  const connectivityResults = ref<Record<string, ConnectivityResult>>({});
  const testingConnectivity = ref<Set<string>>(new Set());

  // Health check + tool management (P5)
  const serverHealth = ref<Record<string, CheckMcpHealthResponse>>({});
  const checkingHealth = ref<Set<string>>(new Set());
  const expandedServers = ref<Set<string>>(new Set());
  const disabledTools = ref<Record<string, Set<string>>>({});

  const runningServers = computed(() => servers.value.filter((s) => s.status === "running"));

  const failedServers = computed(() => servers.value.filter((s) => s.status === "failed"));

  const runningCount = computed(() => runningServers.value.length);

  const hasServers = computed(() => servers.value.length > 0);

  function updateServer(id: string, update: Partial<McpServerEntry>) {
    const idx = servers.value.findIndex((s) => s.id === id);
    if (idx >= 0) {
      Object.assign(servers.value[idx], update);
    } else {
      servers.value.push({
        id,
        status: "stopped",
        tool_count: null,
        ...update
      });
    }
  }

  function upsertSettingsServer(server: McpServerSettingsView): void {
    const existingServerIndex = settingsServers.value.findIndex(
      (settingsServer) => settingsServer.id === server.id
    );
    if (existingServerIndex >= 0) {
      settingsServers.value = settingsServers.value.map((settingsServer) =>
        settingsServer.id === server.id ? server : settingsServer
      );
      return;
    }
    settingsServers.value = [...settingsServers.value, server];
  }

  function updateToolCount(serverId: string, toolCount: number): void {
    settingsServers.value = settingsServers.value.map((server) =>
      server.id === serverId ? { ...server, tool_count: toolCount } : server
    );
    effectiveServers.value = effectiveServers.value.map((server) =>
      server.value.id === serverId
        ? { ...server, value: { ...server.value, tool_count: toolCount } }
        : server
    );
  }

  async function fetchServers(): Promise<void> {
    const ui = useUiStore();
    loading.value = true;
    try {
      const result = await invoke<McpServerStatusResponse[]>("list_mcp_servers");
      servers.value = result.map((s) => ({ ...s, error: undefined }));
    } catch (e) {
      console.error("Failed to fetch MCP servers:", e);
      ui.pushNotification("error", `Failed to fetch MCP servers: ${e}`);
    } finally {
      loading.value = false;
    }
  }

  async function startServer(id: string): Promise<void> {
    const ui = useUiStore();
    try {
      await invoke("start_mcp_server", { serverId: id });
      await fetchServers();
    } catch (e) {
      console.error("Failed to start MCP server:", e);
      ui.pushNotification("error", `Failed to start MCP server: ${e}`);
    }
  }

  async function stopServer(id: string): Promise<void> {
    const ui = useUiStore();
    try {
      await invoke("stop_mcp_server", { serverId: id });
      await fetchServers();
    } catch (e) {
      console.error("Failed to stop MCP server:", e);
      ui.pushNotification("error", `Failed to stop MCP server: ${e}`);
    }
  }

  async function trustServer(id: string): Promise<void> {
    const ui = useUiStore();
    try {
      await invoke("trust_mcp_server", { serverId: id });
      if (!trustedServerIds.value.includes(id)) {
        trustedServerIds.value.push(id);
      }
    } catch (e) {
      console.error("Failed to trust MCP server:", e);
      ui.pushNotification("error", `Failed to trust MCP server: ${e}`);
    }
  }

  async function revokeTrust(id: string): Promise<void> {
    const ui = useUiStore();
    try {
      await invoke("revoke_mcp_trust", { serverId: id });
      trustedServerIds.value = trustedServerIds.value.filter((sid) => sid !== id);
    } catch (e) {
      console.error("Failed to revoke MCP trust:", e);
      ui.pushNotification("error", `Failed to revoke MCP trust: ${e}`);
    }
  }

  async function refreshTools(id: string): Promise<void> {
    const ui = useUiStore();
    try {
      const tools = await invoke<McpToolDefResponse[]>("refresh_mcp_tools", { serverId: id });
      serverHealth.value = {
        ...serverHealth.value,
        [id]: { tools, healthy: true, error: null }
      };
      updateToolCount(id, tools.length);
      await loadDisabledTools(id);
    } catch (e) {
      console.error("Failed to refresh MCP tools:", e);
      ui.pushNotification("error", `Failed to refresh MCP tools: ${e}`);
      serverHealth.value = {
        ...serverHealth.value,
        [id]: { tools: [], healthy: false, error: String(e) }
      };
    }
  }

  async function fetchSettingsServers(sourceFilter?: string | null): Promise<void> {
    settingsLoading.value = true;
    settingsError.value = null;
    try {
      settingsServers.value = await unwrapCommandResult(
        commands.listMcpServerSettings(sourceFilter ?? null)
      );
    } catch (caughtError) {
      settingsError.value = formatError(caughtError);
    } finally {
      settingsLoading.value = false;
    }
  }

  async function saveServerSettings(
    input: McpServerSettingsInput
  ): Promise<McpServerSettingsView | null> {
    settingsLoading.value = true;
    settingsError.value = null;
    try {
      const savedServer = await unwrapCommandResult(commands.upsertMcpServerSettings(input));
      upsertSettingsServer(savedServer);
      await fetchEffectiveServers();
      return savedServer;
    } catch (caughtError) {
      settingsError.value = formatError(caughtError);
      return null;
    } finally {
      settingsLoading.value = false;
    }
  }

  async function setServerEnabled(serverId: string, enabled: boolean): Promise<void> {
    settingsError.value = null;
    try {
      await unwrapCommandResult(commands.setMcpServerEnabled(serverId, enabled));
      settingsServers.value = settingsServers.value.map((settingsServer) =>
        settingsServer.id === serverId ? { ...settingsServer, enabled } : settingsServer
      );
    } catch (caughtError) {
      settingsError.value = formatError(caughtError);
    }
  }

  async function deleteServerSettings(serverId: string): Promise<void> {
    settingsError.value = null;
    try {
      await unwrapCommandResult(commands.deleteMcpServerSettings(serverId));
      settingsServers.value = settingsServers.value.filter(
        (settingsServer) => settingsServer.id !== serverId
      );
    } catch (caughtError) {
      settingsError.value = formatError(caughtError);
    }
  }

  async function disableServerAtScope(serverId: string, projectRoot: string): Promise<void> {
    settingsError.value = null;
    try {
      await unwrapCommandResult(commands.disableMcpServerAtScope(serverId, projectRoot));
      await fetchEffectiveServers();
    } catch (caughtError) {
      settingsError.value = formatError(caughtError);
    }
  }

  async function enableServerAtScope(serverId: string, projectRoot: string): Promise<void> {
    settingsError.value = null;
    try {
      await unwrapCommandResult(commands.enableMcpServerAtScope(serverId, projectRoot));
      await fetchEffectiveServers();
    } catch (caughtError) {
      settingsError.value = formatError(caughtError);
    }
  }

  async function openConfigFile(): Promise<string | null> {
    configFileOpening.value = true;
    settingsError.value = null;
    try {
      return await unwrapCommandResult(commands.openMcpConfigFile());
    } catch (caughtError) {
      settingsError.value = `Unable to open MCP config file: ${formatError(caughtError)}`;
      return null;
    } finally {
      configFileOpening.value = false;
    }
  }

  async function fetchEffectiveServers(): Promise<void> {
    try {
      effectiveServers.value = await unwrapCommandResult(commands.getEffectiveMcpServers());
    } catch (caughtError) {
      settingsError.value = formatError(caughtError);
    }
  }

  async function testConnectivity(serverId: string): Promise<void> {
    const ui = useUiStore();
    const next = new Set(testingConnectivity.value);
    next.add(serverId);
    testingConnectivity.value = next;
    try {
      const result = await commands.testMcpConnectivity(serverId);
      if (result.status === "ok") {
        connectivityResults.value = {
          ...connectivityResults.value,
          [serverId]: result.data
        };
        const server = effectiveServers.value.find((s) => s.value.id === serverId);
        const name = server?.value.name ?? serverId;
        if (result.data.status === "connected") {
          ui.pushNotification(
            "success",
            `MCP server "${name}" connected (${result.data.tool_count} tools)`
          );
        } else {
          ui.pushNotification(
            "error",
            `MCP server "${name}" connectivity test failed: ${result.data.reason}`
          );
        }
      } else {
        connectivityResults.value = {
          ...connectivityResults.value,
          [serverId]: { status: "failed", reason: String(result.error) }
        };
        const server = effectiveServers.value.find((s) => s.value.id === serverId);
        const name = server?.value.name ?? serverId;
        ui.pushNotification(
          "error",
          `MCP server "${name}" connectivity test failed: ${String(result.error)}`
        );
      }
    } catch (e) {
      connectivityResults.value = {
        ...connectivityResults.value,
        [serverId]: { status: "failed", reason: String(e) }
      };
      const server = effectiveServers.value.find((s) => s.value.id === serverId);
      const name = server?.value.name ?? serverId;
      ui.pushNotification("error", `MCP server "${name}" connectivity test failed: ${String(e)}`);
    } finally {
      const next2 = new Set(testingConnectivity.value);
      next2.delete(serverId);
      testingConnectivity.value = next2;
    }
  }

  async function testAllConnectivity(): Promise<void> {
    for (const server of effectiveServers.value) {
      if (server.value.transport !== "builtin") {
        await testConnectivity(server.value.id);
      }
    }
  }

  // ── Health check + tool management ──

  async function checkHealth(serverId: string): Promise<void> {
    const next = new Set(checkingHealth.value);
    next.add(serverId);
    checkingHealth.value = next;

    try {
      const result = await commands.checkMcpHealth(serverId);
      if (result.status === "ok") {
        serverHealth.value = { ...serverHealth.value, [serverId]: result.data };
        updateToolCount(serverId, result.data.tools.length);
        // Load disabled tools state for this server
        await loadDisabledTools(serverId);
      } else {
        serverHealth.value = {
          ...serverHealth.value,
          [serverId]: { tools: [], healthy: false, error: result.error }
        };
      }
    } catch (e) {
      serverHealth.value = {
        ...serverHealth.value,
        [serverId]: { tools: [], healthy: false, error: String(e) }
      };
    } finally {
      const next2 = new Set(checkingHealth.value);
      next2.delete(serverId);
      checkingHealth.value = next2;
    }
  }

  async function checkAllHealth(): Promise<void> {
    const servers = effectiveServers.value.filter(
      (s) => s.value.transport !== "builtin" && s.enabled
    );
    await Promise.all(servers.map((s) => checkHealth(s.value.id)));
  }

  async function refreshAllTools(): Promise<void> {
    const servers = effectiveServers.value.filter(
      (server) => server.value.transport !== "builtin" && server.enabled
    );
    await Promise.all(servers.map((server) => refreshTools(server.value.id)));
  }

  async function refreshInstalledServers(
    sourceFilter?: string | null,
    options: RefreshInstalledOptions = {}
  ): Promise<void> {
    await fetchSettingsServers(sourceFilter ?? null);
    await fetchEffectiveServers();
    if (options.forceTools) {
      await refreshAllTools();
      return;
    }
    await checkAllHealth();
  }

  async function loadDisabledTools(serverId: string): Promise<void> {
    try {
      const result = await commands.getMcpToolStates(serverId);
      if (result.status === "ok") {
        disabledTools.value = {
          ...disabledTools.value,
          [serverId]: new Set(result.data.disabled_tools)
        };
      }
    } catch {
      // Ignore errors loading disabled tools
    }
  }

  function isToolDisabled(serverId: string, toolName: string): boolean {
    return disabledTools.value[serverId]?.has(toolName) ?? false;
  }

  async function setToolDisabled(
    serverId: string,
    toolName: string,
    disabled: boolean
  ): Promise<void> {
    try {
      await commands.setMcpToolDisabled(serverId, toolName, disabled);
      // Update local state
      const current = new Set(disabledTools.value[serverId] ?? []);
      if (disabled) {
        current.add(toolName);
      } else {
        current.delete(toolName);
      }
      disabledTools.value = { ...disabledTools.value, [serverId]: current };
    } catch (e) {
      const ui = useUiStore();
      ui.pushNotification("error", `Failed to ${disabled ? "disable" : "enable"} tool: ${e}`);
    }
  }

  function toggleExpanded(serverId: string): void {
    const next = new Set(expandedServers.value);
    if (next.has(serverId)) {
      next.delete(serverId);
    } else {
      next.add(serverId);
    }
    expandedServers.value = next;
  }

  /**
   * Apply an MCP-related DomainEvent to the local state.
   * Called from useTauriEvents for real-time updates.
   */
  function handleMcpEvent(payload: { type: string; [key: string]: unknown }): void {
    switch (payload.type) {
      case "McpServerStarting":
        updateServer(payload.server_id as string, { status: "starting" });
        break;
      case "McpServerReady":
        updateServer(payload.server_id as string, {
          status: "running",
          tool_count: payload.tool_count as number | null
        });
        break;
      case "McpServerStopped":
        updateServer(payload.server_id as string, {
          status: "stopped",
          tool_count: null
        });
        break;
      case "McpServerFailed":
        updateServer(payload.server_id as string, {
          status: "failed",
          error: payload.error as string
        });
        break;
      case "McpTrustGranted":
        if (!trustedServerIds.value.includes(payload.server_id as string)) {
          trustedServerIds.value.push(payload.server_id as string);
        }
        break;
      case "McpTrustRevoked":
        trustedServerIds.value = trustedServerIds.value.filter(
          (sid) => sid !== (payload.server_id as string)
        );
        break;
    }
  }

  return {
    servers,
    trustedServerIds,
    loading,
    settingsServers,
    settingsLoading,
    configFileOpening,
    settingsError,
    runningServers,
    failedServers,
    runningCount,
    hasServers,
    fetchServers,
    startServer,
    stopServer,
    trustServer,
    revokeTrust,
    refreshTools,
    fetchSettingsServers,
    refreshInstalledServers,
    saveServerSettings,
    setServerEnabled,
    deleteServerSettings,
    disableServerAtScope,
    enableServerAtScope,
    openConfigFile,
    effectiveServers,
    fetchEffectiveServers,
    connectivityResults,
    testingConnectivity,
    testConnectivity,
    testAllConnectivity,
    handleMcpEvent,
    // Health check + tool management
    serverHealth,
    checkingHealth,
    expandedServers,
    disabledTools,
    checkHealth,
    checkAllHealth,
    isToolDisabled,
    setToolDisabled,
    toggleExpanded
  };
});

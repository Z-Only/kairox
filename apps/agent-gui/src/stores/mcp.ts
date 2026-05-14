// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Pinia stores are plain `.ts` modules and
// must import `defineStore`, `ref`, and `computed` explicitly.
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
  commands,
  type EffectiveMcpServerView,
  type McpServerSettingsInput,
  type McpServerSettingsView,
  type McpServerStatusResponse
} from "@/generated/commands";
import { useUiStore } from "@/stores/ui";

export interface McpServerEntry extends McpServerStatusResponse {
  error?: string;
}

type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: string };

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
      await invoke("refresh_mcp_tools", { serverId: id });
      await fetchServers();
    } catch (e) {
      console.error("Failed to refresh MCP tools:", e);
      ui.pushNotification("error", `Failed to refresh MCP tools: ${e}`);
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
    saveServerSettings,
    setServerEnabled,
    deleteServerSettings,
    openConfigFile,
    effectiveServers,
    fetchEffectiveServers,
    handleMcpEvent
  };
});

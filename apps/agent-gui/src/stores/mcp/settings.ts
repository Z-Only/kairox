import {
  commands,
  type McpServerSettingsInput,
  type McpServerSettingsView
} from "@/generated/commands";
import type { McpState } from "./state";
import { formatError, unwrapCommandResult } from "./utils";

function diagnosticSummaryFor(server: McpServerSettingsView): string {
  const trust = server.trusted ? "trusted" : "untrusted";
  const tools =
    server.tool_count === 1
      ? "1 tool"
      : typeof server.tool_count === "number"
        ? `${server.tool_count} tools`
        : "unknown";
  const verification = server.verified ? "verified" : "unverified";
  const error = server.last_error ?? "none";
  return `status: ${server.runtime_status}; trust: ${trust}; tools: ${tools}; ${verification}; error: ${error}`;
}

function withToolCount(server: McpServerSettingsView, toolCount: number): McpServerSettingsView {
  const updated = { ...server, tool_count: toolCount };
  return { ...updated, diagnostic_summary: diagnosticSummaryFor(updated) };
}

export function createSettings(state: McpState) {
  const { settingsServers, settingsLoading, configFileOpening, settingsError, effectiveServers } =
    state;

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
      server.id === serverId ? withToolCount(server, toolCount) : server
    );
    effectiveServers.value = effectiveServers.value.map((server) =>
      server.value.id === serverId
        ? { ...server, value: withToolCount(server.value, toolCount) }
        : server
    );
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

  async function fetchEffectiveServers(): Promise<void> {
    try {
      effectiveServers.value = await unwrapCommandResult(commands.getEffectiveMcpServers());
    } catch (caughtError) {
      settingsError.value = formatError(caughtError);
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

  return {
    upsertSettingsServer,
    updateToolCount,
    fetchSettingsServers,
    fetchEffectiveServers,
    saveServerSettings,
    setServerEnabled,
    deleteServerSettings,
    disableServerAtScope,
    enableServerAtScope,
    openConfigFile
  };
}

export type McpSettingsActions = ReturnType<typeof createSettings>;

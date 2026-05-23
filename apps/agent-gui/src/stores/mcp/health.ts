import { commands } from "@/generated/commands";
import type { McpState } from "./state";
import type { RefreshInstalledOptions } from "./types";

export interface HealthDeps {
  updateToolCount: (serverId: string, toolCount: number) => void;
  loadDisabledTools: (serverId: string) => Promise<void>;
  refreshTools: (serverId: string) => Promise<void>;
  fetchSettingsServers: (sourceFilter?: string | null) => Promise<void>;
  fetchEffectiveServers: () => Promise<void>;
}

export function createHealth(state: McpState, deps: HealthDeps) {
  const { serverHealth, checkingHealth, effectiveServers } = state;
  const {
    updateToolCount,
    loadDisabledTools,
    refreshTools,
    fetchSettingsServers,
    fetchEffectiveServers
  } = deps;

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

  return {
    checkHealth,
    checkAllHealth,
    refreshAllTools,
    refreshInstalledServers
  };
}

export type McpHealthActions = ReturnType<typeof createHealth>;

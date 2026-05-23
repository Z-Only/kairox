import { invoke } from "@tauri-apps/api/core";
import type { McpServerStatusResponse, McpToolDefResponse } from "@/generated/commands";
import { useUiStore } from "@/stores/ui";
import type { McpState } from "./state";
import type { McpServerEntry } from "./types";

export interface LifecycleDeps {
  updateToolCount: (serverId: string, toolCount: number) => void;
  loadDisabledTools: (serverId: string) => Promise<void>;
}

export function createLifecycle(state: McpState, deps: LifecycleDeps) {
  const { servers, trustedServerIds, loading, serverHealth } = state;
  const { updateToolCount, loadDisabledTools } = deps;

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
    fetchServers,
    startServer,
    stopServer,
    trustServer,
    revokeTrust,
    refreshTools,
    handleMcpEvent
  };
}

export type McpLifecycleActions = ReturnType<typeof createLifecycle>;

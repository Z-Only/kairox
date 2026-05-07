import { reactive, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { McpServerStatusResponse } from "../generated/commands";
import { addNotification } from "../composables/useNotifications";

export interface McpServerEntry extends McpServerStatusResponse {
  error?: string;
}

export const mcpState = reactive({
  servers: [] as McpServerEntry[],
  trustedServerIds: [] as string[],
  loading: false
});

export const runningServers = computed(() =>
  mcpState.servers.filter((s) => s.status === "running")
);

export const failedServers = computed(() => mcpState.servers.filter((s) => s.status === "failed"));

export const runningCount = computed(() => runningServers.value.length);

export const hasServers = computed(() => mcpState.servers.length > 0);

function updateServer(id: string, update: Partial<McpServerEntry>) {
  const idx = mcpState.servers.findIndex((s) => s.id === id);
  if (idx >= 0) {
    Object.assign(mcpState.servers[idx], update);
  } else {
    mcpState.servers.push({
      id,
      status: "stopped",
      tool_count: null,
      ...update
    });
  }
}

export async function fetchServers(): Promise<void> {
  mcpState.loading = true;
  try {
    const result = await invoke<McpServerStatusResponse[]>("list_mcp_servers");
    mcpState.servers = result.map((s) => ({ ...s, error: undefined }));
  } catch (e) {
    console.error("Failed to fetch MCP servers:", e);
    addNotification("error", `Failed to fetch MCP servers: ${e}`);
  } finally {
    mcpState.loading = false;
  }
}

export async function startServer(id: string): Promise<void> {
  try {
    await invoke("start_mcp_server", { serverId: id });
    await fetchServers();
  } catch (e) {
    console.error("Failed to start MCP server:", e);
    addNotification("error", `Failed to start MCP server: ${e}`);
  }
}

export async function stopServer(id: string): Promise<void> {
  try {
    await invoke("stop_mcp_server", { serverId: id });
    await fetchServers();
  } catch (e) {
    console.error("Failed to stop MCP server:", e);
    addNotification("error", `Failed to stop MCP server: ${e}`);
  }
}

export async function trustServer(id: string): Promise<void> {
  try {
    await invoke("trust_mcp_server", { serverId: id });
    if (!mcpState.trustedServerIds.includes(id)) {
      mcpState.trustedServerIds.push(id);
    }
  } catch (e) {
    console.error("Failed to trust MCP server:", e);
    addNotification("error", `Failed to trust MCP server: ${e}`);
  }
}

export async function revokeTrust(id: string): Promise<void> {
  try {
    await invoke("revoke_mcp_trust", { serverId: id });
    mcpState.trustedServerIds = mcpState.trustedServerIds.filter((sid) => sid !== id);
  } catch (e) {
    console.error("Failed to revoke MCP trust:", e);
    addNotification("error", `Failed to revoke MCP trust: ${e}`);
  }
}

export async function refreshTools(id: string): Promise<void> {
  try {
    await invoke("refresh_mcp_tools", { serverId: id });
    await fetchServers();
  } catch (e) {
    console.error("Failed to refresh MCP tools:", e);
    addNotification("error", `Failed to refresh MCP tools: ${e}`);
  }
}

/**
 * Apply an MCP-related DomainEvent to the local state.
 * Called from useTauriEvents for real-time updates.
 */
export function handleMcpEvent(payload: { type: string; [key: string]: unknown }): void {
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
      if (!mcpState.trustedServerIds.includes(payload.server_id as string)) {
        mcpState.trustedServerIds.push(payload.server_id as string);
      }
      break;
    case "McpTrustRevoked":
      mcpState.trustedServerIds = mcpState.trustedServerIds.filter(
        (sid) => sid !== (payload.server_id as string)
      );
      break;
  }
}

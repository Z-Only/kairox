import { commands } from "@/generated/commands";
import { useUiStore } from "@/stores/ui";
import type { McpState } from "./state";

export function createConnectivity(state: McpState) {
  const { connectivityResults, testingConnectivity, effectiveServers } = state;

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

  return {
    testConnectivity,
    testAllConnectivity
  };
}

export type McpConnectivityActions = ReturnType<typeof createConnectivity>;

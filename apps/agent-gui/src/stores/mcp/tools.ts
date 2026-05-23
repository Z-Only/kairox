import { commands } from "@/generated/commands";
import { useUiStore } from "@/stores/ui";
import type { McpState } from "./state";

export function createTools(state: McpState) {
  const { disabledTools, expandedServers } = state;

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

  return {
    loadDisabledTools,
    isToolDisabled,
    setToolDisabled,
    toggleExpanded
  };
}

export type McpToolsActions = ReturnType<typeof createTools>;

import { commands, type McpContentBlockResponse } from "@/generated/commands";
import type { McpState } from "./state";

export function createResources(state: McpState) {
  const {
    serverResources,
    serverPrompts,
    loadingResources,
    loadingPrompts,
    expandedResourceUri,
    resourcesError,
    promptsError,
    resourceContentCache
  } = state;

  async function fetchResources(serverId: string): Promise<void> {
    if (serverResources.value[serverId]) return;
    const next = new Set(loadingResources.value);
    next.add(serverId);
    loadingResources.value = next;
    try {
      const result = await commands.listMcpResources(serverId);
      if (result.status === "ok") {
        serverResources.value = { ...serverResources.value, [serverId]: result.data };
        resourcesError.value = { ...resourcesError.value, [serverId]: null };
      } else {
        resourcesError.value = { ...resourcesError.value, [serverId]: result.error };
      }
    } catch (e) {
      resourcesError.value = { ...resourcesError.value, [serverId]: String(e) };
    } finally {
      const next2 = new Set(loadingResources.value);
      next2.delete(serverId);
      loadingResources.value = next2;
    }
  }

  async function fetchPrompts(serverId: string): Promise<void> {
    if (serverPrompts.value[serverId]) return;
    const next = new Set(loadingPrompts.value);
    next.add(serverId);
    loadingPrompts.value = next;
    try {
      const result = await commands.listMcpPrompts(serverId);
      if (result.status === "ok") {
        serverPrompts.value = { ...serverPrompts.value, [serverId]: result.data };
        promptsError.value = { ...promptsError.value, [serverId]: null };
      } else {
        promptsError.value = { ...promptsError.value, [serverId]: result.error };
      }
    } catch (e) {
      promptsError.value = { ...promptsError.value, [serverId]: String(e) };
    } finally {
      const next2 = new Set(loadingPrompts.value);
      next2.delete(serverId);
      loadingPrompts.value = next2;
    }
  }

  async function readResource(serverId: string, uri: string): Promise<McpContentBlockResponse[]> {
    const cacheKey = `${serverId}:${uri}`;
    if (resourceContentCache.value[cacheKey]) return resourceContentCache.value[cacheKey];
    const result = await commands.readMcpResource(serverId, uri);
    if (result.status === "ok") {
      resourceContentCache.value = { ...resourceContentCache.value, [cacheKey]: result.data };
      return result.data;
    }
    throw new Error(result.error);
  }

  function toggleResourceExpand(serverId: string, uri: string): void {
    const current = expandedResourceUri.value[serverId];
    expandedResourceUri.value = {
      ...expandedResourceUri.value,
      [serverId]: current === uri ? null : uri
    };
  }

  return {
    fetchResources,
    fetchPrompts,
    readResource,
    toggleResourceExpand
  };
}

export type McpResourcesActions = ReturnType<typeof createResources>;

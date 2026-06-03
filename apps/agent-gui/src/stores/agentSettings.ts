import { computed, ref } from "vue";
import { defineStore } from "pinia";
import {
  commands,
  type AgentSettingsInput,
  type AgentSettingsScope,
  type AgentSettingsView
} from "@/generated/commands";

type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: string };

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
  if (!isCommandResult(result)) return result;
  if (result.status === "error") throw new Error(result.error);
  return result.data;
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export const useAgentSettingsStore = defineStore("agentSettings", () => {
  const agents = ref<AgentSettingsView[]>([]);
  const loading = ref(false);
  const saving = ref(false);
  const error = ref<string | null>(null);

  const effectiveAgents = computed(() => agents.value.filter((agent) => agent.effective));

  function normalizeProjectRoot(projectRoot?: string | null): string | null {
    const trimmed = projectRoot?.trim();
    return trimmed ? trimmed : null;
  }

  async function loadAgents(projectRoot?: string | null): Promise<void> {
    const normalizedProjectRoot = normalizeProjectRoot(projectRoot);
    loading.value = true;
    error.value = null;
    try {
      agents.value = await unwrapCommandResult(commands.listAgentSettings(normalizedProjectRoot));
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      loading.value = false;
    }
  }

  async function saveAgent(
    input: AgentSettingsInput,
    projectRoot?: string | null
  ): Promise<AgentSettingsView | null> {
    const normalizedProjectRoot = normalizeProjectRoot(projectRoot);
    saving.value = true;
    error.value = null;
    try {
      const saved = await unwrapCommandResult(
        commands.upsertAgentSettings(input, normalizedProjectRoot)
      );
      await loadAgents(normalizedProjectRoot);
      return saved;
    } catch (caughtError) {
      error.value = formatError(caughtError);
      return null;
    } finally {
      saving.value = false;
    }
  }

  async function deleteAgent(agentId: string, projectRoot?: string | null): Promise<void> {
    const normalizedProjectRoot = normalizeProjectRoot(projectRoot);
    error.value = null;
    await unwrapCommandResult(commands.deleteAgentSettings(agentId, normalizedProjectRoot));
    await loadAgents(normalizedProjectRoot);
  }

  async function copyAgent(
    agentId: string,
    scope: AgentSettingsScope,
    projectRoot?: string | null
  ): Promise<void> {
    const normalizedProjectRoot = normalizeProjectRoot(projectRoot);
    error.value = null;
    await unwrapCommandResult(commands.copyAgentSettings(agentId, scope, normalizedProjectRoot));
    await loadAgents(normalizedProjectRoot);
  }

  async function openAgentsDir(projectRoot?: string | null): Promise<void> {
    try {
      await commands.openAgentsDir(normalizeProjectRoot(projectRoot));
    } catch {
      // best-effort opener
    }
  }

  return {
    agents,
    loading,
    saving,
    error,
    effectiveAgents,
    loadAgents,
    saveAgent,
    deleteAgent,
    copyAgent,
    openAgentsDir
  };
});

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

  async function loadAgents(): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      agents.value = await unwrapCommandResult(commands.listAgentSettings());
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      loading.value = false;
    }
  }

  async function saveAgent(input: AgentSettingsInput): Promise<AgentSettingsView | null> {
    saving.value = true;
    error.value = null;
    try {
      const saved = await unwrapCommandResult(commands.upsertAgentSettings(input));
      await loadAgents();
      return saved;
    } catch (caughtError) {
      error.value = formatError(caughtError);
      return null;
    } finally {
      saving.value = false;
    }
  }

  async function deleteAgent(agentId: string): Promise<void> {
    error.value = null;
    await unwrapCommandResult(commands.deleteAgentSettings(agentId));
    await loadAgents();
  }

  async function copyAgent(agentId: string, scope: AgentSettingsScope): Promise<void> {
    error.value = null;
    await unwrapCommandResult(commands.copyAgentSettings(agentId, scope));
    await loadAgents();
  }

  async function openAgentsDir(): Promise<void> {
    try {
      await commands.openAgentsDir();
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

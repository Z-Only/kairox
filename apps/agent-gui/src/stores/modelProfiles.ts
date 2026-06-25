import { ref } from "vue";
import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import {
  commands,
  type ConnectivityTestResult,
  type ProfileSettingsView,
  type ProfileSettingsInput
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

export function formatError(caughtError: unknown): string {
  return caughtError instanceof Error ? caughtError.message : String(caughtError);
}

export interface ModelHealthAdvice {
  tone: "success" | "warning" | "danger";
  label: string;
  recommendation: string;
}

const MODEL_HEALTH_ADVICE_BY_STATUS: Record<string, ModelHealthAdvice> = {
  chat_ready: {
    tone: "success",
    label: "Chat ready",
    recommendation: "Model responded to a chat probe."
  },
  endpoint_reachable: {
    tone: "success",
    label: "Endpoint reachable",
    recommendation: "Endpoint accepted the connectivity probe."
  },
  empty_response: {
    tone: "warning",
    label: "Empty response",
    recommendation: "Check model availability, quota, or plan access."
  },
  auth_failed: {
    tone: "danger",
    label: "Authentication failed",
    recommendation: "Check the API key or configured API key environment variable."
  },
  quota_or_plan_blocked: {
    tone: "danger",
    label: "Quota or plan blocked",
    recommendation: "Check quota, billing, and model access for this account."
  },
  rate_limited: {
    tone: "warning",
    label: "Rate limited",
    recommendation: "Wait and retry, or reduce request rate."
  },
  network_error: {
    tone: "warning",
    label: "Network error",
    recommendation: "Check network connectivity and the endpoint URL."
  }
};

export function modelHealthAdvice(result: ConnectivityTestResult): ModelHealthAdvice {
  const knownAdvice = MODEL_HEALTH_ADVICE_BY_STATUS[result.status];
  if (knownAdvice) return knownAdvice;
  if (result.ok) {
    return {
      tone: "success",
      label: "Connectivity check passed",
      recommendation: "The profile responded successfully."
    };
  }
  return {
    tone: "danger",
    label: "Connectivity check failed",
    recommendation: "Review the raw error and model configuration."
  };
}

export const useModelProfilesStore = defineStore("modelProfiles", () => {
  const profiles = ref<ProfileSettingsView[]>([]);
  const loading = ref(false);
  const refreshing = ref(false);
  const error = ref<string | null>(null);
  const busyAlias = ref<string | null>(null);

  async function loadProfiles(
    sourceFilter?: string | null,
    projectRoot?: string | null
  ): Promise<void> {
    const isInitialLoad = profiles.value.length === 0;
    if (isInitialLoad) {
      loading.value = true;
    } else {
      refreshing.value = true;
    }
    error.value = null;
    try {
      profiles.value = await unwrapCommandResult(
        commands.listProfileSettings(sourceFilter ?? null, projectRoot ?? null)
      );
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      loading.value = false;
      refreshing.value = false;
    }
  }

  async function refreshRuntime(projectRoot?: string | null): Promise<void> {
    try {
      if (projectRoot) {
        await commands.refreshConfigForProject(projectRoot);
      } else {
        await commands.refreshConfig();
      }
    } catch {
      // best-effort: runtime refresh failure should not block list display
    }
  }

  async function upsertProfile(input: ProfileSettingsInput): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      await unwrapCommandResult(commands.upsertProfileSettings(input));
      await loadProfiles();
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      loading.value = false;
    }
  }

  async function setProfileEnabled(alias: string, enabled: boolean): Promise<void> {
    busyAlias.value = alias;
    error.value = null;
    try {
      await unwrapCommandResult(commands.setProfileEnabled(alias, enabled));
      await loadProfiles();
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      busyAlias.value = null;
    }
  }

  async function removeProfile(alias: string): Promise<void> {
    busyAlias.value = alias;
    error.value = null;
    try {
      await unwrapCommandResult(commands.deleteProfileSettings(alias));
      await loadProfiles();
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      busyAlias.value = null;
    }
  }

  async function moveProfile(alias: string, direction: number): Promise<void> {
    busyAlias.value = alias;
    error.value = null;
    try {
      await unwrapCommandResult(commands.moveProfileInOrder(alias, direction));
      await loadProfiles();
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      busyAlias.value = null;
    }
  }

  async function testModelConnectivity(alias: string, projectRoot?: string | null) {
    busyAlias.value = alias;
    try {
      return await commands.testModelConnectivity(alias, projectRoot ?? null);
    } finally {
      busyAlias.value = null;
    }
  }

  async function testUrlConnectivity(url: string) {
    return commands.testUrlConnectivity(url);
  }

  async function openConfigFile(scope?: string, projectRoot?: string | null): Promise<void> {
    try {
      if (scope === "project" && projectRoot) {
        await invoke("open_config_file_for_scope", { scope: "project", projectRoot });
      } else {
        await invoke("open_config_file_for_scope", { scope: "user", projectRoot: null });
      }
    } catch {
      // best-effort
    }
  }

  return {
    profiles,
    loading,
    refreshing,
    error,
    busyAlias,
    loadProfiles,
    refreshRuntime,
    upsertProfile,
    setProfileEnabled,
    removeProfile,
    moveProfile,
    testModelConnectivity,
    testUrlConnectivity,
    openConfigFile
  };
});

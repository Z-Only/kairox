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

type HealthAdviceTranslator = (key: string) => string;

interface ModelHealthAdviceDefinition extends ModelHealthAdvice {
  labelKey: string;
  recommendationKey: string;
}

const MODEL_HEALTH_ADVICE_BY_STATUS: Record<string, ModelHealthAdviceDefinition> = {
  chat_ready: {
    tone: "success",
    labelKey: "models.healthAdvice_chat_ready_label",
    label: "Chat ready",
    recommendationKey: "models.healthAdvice_chat_ready_recommendation",
    recommendation: "Model responded to a chat probe."
  },
  endpoint_reachable: {
    tone: "success",
    labelKey: "models.healthAdvice_endpoint_reachable_label",
    label: "Endpoint reachable",
    recommendationKey: "models.healthAdvice_endpoint_reachable_recommendation",
    recommendation: "Endpoint accepted the connectivity probe."
  },
  empty_response: {
    tone: "warning",
    labelKey: "models.healthAdvice_empty_response_label",
    label: "Empty response",
    recommendationKey: "models.healthAdvice_empty_response_recommendation",
    recommendation: "Check model availability, quota, or plan access."
  },
  auth_failed: {
    tone: "danger",
    labelKey: "models.healthAdvice_auth_failed_label",
    label: "Authentication failed",
    recommendationKey: "models.healthAdvice_auth_failed_recommendation",
    recommendation: "Check the API key or configured API key environment variable."
  },
  quota_or_plan_blocked: {
    tone: "danger",
    labelKey: "models.healthAdvice_quota_or_plan_blocked_label",
    label: "Quota or plan blocked",
    recommendationKey: "models.healthAdvice_quota_or_plan_blocked_recommendation",
    recommendation: "Check quota, billing, and model access for this account."
  },
  rate_limited: {
    tone: "warning",
    labelKey: "models.healthAdvice_rate_limited_label",
    label: "Rate limited",
    recommendationKey: "models.healthAdvice_rate_limited_recommendation",
    recommendation: "Wait and retry, or reduce request rate."
  },
  network_error: {
    tone: "warning",
    labelKey: "models.healthAdvice_network_error_label",
    label: "Network error",
    recommendationKey: "models.healthAdvice_network_error_recommendation",
    recommendation: "Check network connectivity and the endpoint URL."
  },
  permission_denied: {
    tone: "danger",
    labelKey: "models.healthAdvice_permission_denied_label",
    label: "Permission denied",
    recommendationKey: "models.healthAdvice_permission_denied_recommendation",
    recommendation: "Use an API key with access to this model or endpoint."
  },
  model_unavailable: {
    tone: "danger",
    labelKey: "models.healthAdvice_model_unavailable_label",
    label: "Model unavailable",
    recommendationKey: "models.healthAdvice_model_unavailable_recommendation",
    recommendation: "Check the model ID, provider, and account access."
  },
  server_error: {
    tone: "warning",
    labelKey: "models.healthAdvice_server_error_label",
    label: "Server error",
    recommendationKey: "models.healthAdvice_server_error_recommendation",
    recommendation: "Retry later or check provider status."
  },
  invalid_config: {
    tone: "danger",
    labelKey: "models.healthAdvice_invalid_config_label",
    label: "Invalid configuration",
    recommendationKey: "models.healthAdvice_invalid_config_recommendation",
    recommendation: "Check provider, base URL, API key settings, and model ID."
  },
  request_failed: {
    tone: "danger",
    labelKey: "models.healthAdvice_request_failed_label",
    label: "Request failed",
    recommendationKey: "models.healthAdvice_request_failed_recommendation",
    recommendation: "Review the raw error and model configuration."
  }
};

const PASSED_HEALTH_ADVICE: ModelHealthAdviceDefinition = {
  tone: "success",
  labelKey: "models.healthAdvice_passed_label",
  label: "Connectivity check passed",
  recommendationKey: "models.healthAdvice_passed_recommendation",
  recommendation: "The profile responded successfully."
};

const FAILED_HEALTH_ADVICE: ModelHealthAdviceDefinition = {
  tone: "danger",
  labelKey: "models.healthAdvice_failed_label",
  label: "Connectivity check failed",
  recommendationKey: "models.healthAdvice_failed_recommendation",
  recommendation: "Review the raw error and model configuration."
};

function resolveModelHealthAdvice(
  advice: ModelHealthAdviceDefinition,
  translate?: HealthAdviceTranslator
): ModelHealthAdvice {
  return {
    tone: advice.tone,
    label: translate ? translate(advice.labelKey) : advice.label,
    recommendation: translate ? translate(advice.recommendationKey) : advice.recommendation
  };
}

export function modelHealthAdvice(
  result: ConnectivityTestResult,
  translate?: HealthAdviceTranslator
): ModelHealthAdvice {
  const knownAdvice = MODEL_HEALTH_ADVICE_BY_STATUS[result.status];
  if (knownAdvice) return resolveModelHealthAdvice(knownAdvice, translate);
  if (result.ok) {
    return resolveModelHealthAdvice(PASSED_HEALTH_ADVICE, translate);
  }
  return resolveModelHealthAdvice(FAILED_HEALTH_ADVICE, translate);
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

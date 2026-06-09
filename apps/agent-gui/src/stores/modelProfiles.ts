import { ref } from "vue";
import { defineStore } from "pinia";
import {
  commands,
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

  async function openConfigFile(
    scope?: "user" | "project" | null,
    projectRoot?: string | null
  ): Promise<void> {
    try {
      if (scope === "project" && projectRoot) {
        await commands.openConfigFileForScope("project", projectRoot);
      } else {
        await commands.openConfigFileForScope("user", null);
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

import { computed, ref } from "vue";
import { defineStore } from "pinia";
import {
  commands,
  type InstallPluginRequest,
  type PluginCatalogEntry,
  type PluginInstallTarget,
  type PluginMarketplaceSourceView,
  type PluginSettingsView
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

export const usePluginsStore = defineStore("plugins", () => {
  const plugins = ref<PluginSettingsView[]>([]);
  const sources = ref<PluginMarketplaceSourceView[]>([]);
  const catalog = ref<PluginCatalogEntry[]>([]);
  const loading = ref(false);
  const catalogLoading = ref(false);
  const busyPluginId = ref<string | null>(null);
  const error = ref<string | null>(null);

  const effectivePlugins = computed(() => plugins.value.filter((plugin) => plugin.effective));

  async function loadPlugins(): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      plugins.value = await unwrapCommandResult(commands.listPluginSettings());
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      loading.value = false;
    }
  }

  async function loadSources(): Promise<void> {
    error.value = null;
    try {
      sources.value = await unwrapCommandResult(commands.listPluginMarketplaceSources());
    } catch (caughtError) {
      error.value = formatError(caughtError);
    }
  }

  async function setMarketplaceSourceEnabled(sourceId: string, enabled: boolean): Promise<void> {
    error.value = null;
    try {
      await unwrapCommandResult(commands.setPluginMarketplaceSourceEnabled(sourceId, enabled));
      await loadSources();
      await loadCatalog(null, "");
    } catch (caughtError) {
      error.value = formatError(caughtError);
    }
  }

  async function loadCatalog(marketplaceId: string | null, keyword: string | null): Promise<void> {
    catalogLoading.value = true;
    error.value = null;
    try {
      catalog.value = await unwrapCommandResult(commands.listPluginCatalog(marketplaceId, keyword));
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      catalogLoading.value = false;
    }
  }

  async function setPluginEnabled(settingsId: string, enabled: boolean): Promise<void> {
    busyPluginId.value = settingsId;
    error.value = null;
    try {
      await unwrapCommandResult(commands.setPluginEnabled(settingsId, enabled));
      await loadPlugins();
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      busyPluginId.value = null;
    }
  }

  async function deletePlugin(settingsId: string): Promise<void> {
    busyPluginId.value = settingsId;
    error.value = null;
    try {
      await unwrapCommandResult(commands.deletePluginSettings(settingsId));
      await loadPlugins();
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      busyPluginId.value = null;
    }
  }

  async function installPlugin(
    marketplaceId: string,
    pluginName: string,
    target: PluginInstallTarget
  ): Promise<PluginSettingsView | null> {
    busyPluginId.value = `${marketplaceId}:${pluginName}`;
    error.value = null;
    const request: InstallPluginRequest = {
      marketplace_id: marketplaceId,
      plugin_name: pluginName,
      target
    };
    try {
      const installed = await unwrapCommandResult(commands.installPlugin(request));
      await loadPlugins();
      return installed;
    } catch (caughtError) {
      error.value = formatError(caughtError);
      return null;
    } finally {
      busyPluginId.value = null;
    }
  }

  return {
    plugins,
    sources,
    catalog,
    loading,
    catalogLoading,
    busyPluginId,
    error,
    effectivePlugins,
    loadPlugins,
    loadSources,
    setMarketplaceSourceEnabled,
    loadCatalog,
    setPluginEnabled,
    deletePlugin,
    installPlugin
  };
});

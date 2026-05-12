// `unplugin-auto-import` only injects globals into `.vue` SFCs. Pinia stores
// are plain `.ts` modules and must import Vue and Pinia APIs explicitly.
import { computed, ref } from "vue";
import { defineStore } from "pinia";
import {
  commands,
  type ActiveSkillView,
  type InstallGithubSkillRequest,
  type InstallRemoteSkillRequest,
  type RemoteSkillSearchResult,
  type SkillCatalogEntry,
  type SkillCatalogQuery,
  type SkillDetail,
  type SkillInstallTarget,
  type SkillSettingsView,
  type SkillSourceView,
  type SkillView
} from "@/generated/commands";

export type { ActiveSkillView, SkillDetail, SkillView } from "@/generated/commands";

type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: string };

function formatError(caughtError: unknown): string {
  return caughtError instanceof Error ? caughtError.message : String(caughtError);
}

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
  if (!isCommandResult(result)) {
    return result;
  }
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

export const useSkillsStore = defineStore("skills", () => {
  const skills = ref<SkillView[]>([]);
  const activeSkills = ref<ActiveSkillView[]>([]);
  const selectedSkill = ref<SkillDetail | null>(null);
  const loading = ref(false);
  const activatingSkillId = ref<string | null>(null);
  const error = ref<string | null>(null);
  const skillSettings = ref<SkillSettingsView[]>([]);
  const remoteResults = ref<RemoteSkillSearchResult[]>([]);
  const settingsLoading = ref(false);
  const remoteLoading = ref(false);

  // Skill catalog / marketplace
  const catalogEntries = ref<SkillCatalogEntry[]>([]);
  const catalogSources = ref<SkillSourceView[]>([]);
  const catalogLoading = ref(false);
  const searchCache = new Map<string, { entries: SkillCatalogEntry[]; timestamp: number }>();
  const CACHE_TTL_MS = 5 * 60 * 1000; // 5 minutes

  const hasSkills = computed(() => skills.value.length > 0);
  const activeSkillIds = computed(() =>
    activeSkills.value.map((activeSkill) => activeSkill.skill_id)
  );

  function isSkillActive(skillId: string): boolean {
    return activeSkillIds.value.includes(skillId);
  }

  function upsertSkillSetting(skillSetting: SkillSettingsView): void {
    const existingSkillIndex = skillSettings.value.findIndex(
      (existingSkill) => existingSkill.settings_id === skillSetting.settings_id
    );
    if (existingSkillIndex >= 0) {
      skillSettings.value = skillSettings.value.map((existingSkill) =>
        existingSkill.settings_id === skillSetting.settings_id ? skillSetting : existingSkill
      );
      return;
    }
    skillSettings.value = [...skillSettings.value, skillSetting];
  }

  async function loadSkills(): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      const [discoveredSkills, activeSkillViews] = await Promise.all([
        unwrapCommandResult(commands.listSkills()),
        unwrapCommandResult(commands.listActiveSkills())
      ]);
      skills.value = discoveredSkills;
      activeSkills.value = activeSkillViews;
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      loading.value = false;
    }
  }

  async function loadSkillDetail(skillId: string): Promise<void> {
    error.value = null;
    try {
      selectedSkill.value = await unwrapCommandResult(commands.getSkillDetail(skillId));
    } catch (caughtError) {
      error.value = formatError(caughtError);
    }
  }

  async function activateSkill(skillId: string): Promise<void> {
    activatingSkillId.value = skillId;
    error.value = null;
    try {
      const activeSkill = await unwrapCommandResult(commands.activateSkill(skillId));
      activeSkills.value = [
        ...activeSkills.value.filter((existingSkill) => existingSkill.skill_id !== skillId),
        activeSkill
      ];
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      activatingSkillId.value = null;
    }
  }

  async function deactivateSkill(skillId: string): Promise<void> {
    activatingSkillId.value = skillId;
    error.value = null;
    try {
      await unwrapCommandResult(commands.deactivateSkill(skillId));
      activeSkills.value = activeSkills.value.filter(
        (activeSkill) => activeSkill.skill_id !== skillId
      );
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      activatingSkillId.value = null;
    }
  }

  async function loadSkillSettings(): Promise<void> {
    settingsLoading.value = true;
    error.value = null;
    try {
      skillSettings.value = await unwrapCommandResult(commands.listSkillSettings());
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      settingsLoading.value = false;
    }
  }

  async function setSkillEnabled(skillSettingsId: string, enabled: boolean): Promise<void> {
    error.value = null;
    try {
      await unwrapCommandResult(commands.setSkillEnabled(skillSettingsId, enabled));
      skillSettings.value = skillSettings.value.map((skillSetting) =>
        skillSetting.settings_id === skillSettingsId ? { ...skillSetting, enabled } : skillSetting
      );
    } catch (caughtError) {
      error.value = formatError(caughtError);
    }
  }

  async function deleteSkill(skillSettingsId: string): Promise<void> {
    error.value = null;
    try {
      await unwrapCommandResult(commands.deleteSkillSettings(skillSettingsId));
      skillSettings.value = skillSettings.value.filter(
        (skillSetting) => skillSetting.settings_id !== skillSettingsId
      );
    } catch (caughtError) {
      error.value = formatError(caughtError);
    }
  }

  async function searchRemoteSkills(query: string): Promise<void> {
    remoteLoading.value = true;
    error.value = null;
    try {
      remoteResults.value = await unwrapCommandResult(commands.searchRemoteSkills(query));
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      remoteLoading.value = false;
    }
  }

  async function installRemoteSkill(
    packageName: string,
    target: SkillInstallTarget
  ): Promise<SkillSettingsView | null> {
    settingsLoading.value = true;
    error.value = null;
    const request: InstallRemoteSkillRequest = {
      package: packageName,
      source: packageName,
      target
    };
    try {
      const installedSkill = await unwrapCommandResult(commands.installRemoteSkill(request));
      upsertSkillSetting(installedSkill);
      return installedSkill;
    } catch (caughtError) {
      error.value = formatError(caughtError);
      return null;
    } finally {
      settingsLoading.value = false;
    }
  }

  async function installGithubSkill(
    source: string,
    target: SkillInstallTarget
  ): Promise<SkillSettingsView | null> {
    settingsLoading.value = true;
    error.value = null;
    const request: InstallGithubSkillRequest = { source, target };
    try {
      const installedSkill = await unwrapCommandResult(commands.installGithubSkill(request));
      upsertSkillSetting(installedSkill);
      return installedSkill;
    } catch (caughtError) {
      error.value = formatError(caughtError);
      return null;
    } finally {
      settingsLoading.value = false;
    }
  }

  async function updateSkill(skillId: string): Promise<SkillSettingsView | null> {
    settingsLoading.value = true;
    error.value = null;
    try {
      const updatedSkill = await unwrapCommandResult(commands.updateSkill(skillId));
      upsertSkillSetting(updatedSkill);
      return updatedSkill;
    } catch (caughtError) {
      error.value = formatError(caughtError);
      return null;
    } finally {
      settingsLoading.value = false;
    }
  }

  function cacheKeyForQuery(query: SkillCatalogQuery): string {
    const keyword = query.keyword ?? "";
    const sources = query.sources?.join(",") ?? "";
    return `${keyword}|${sources}|${query.limit ?? 50}`;
  }

  async function searchCatalog(query: SkillCatalogQuery): Promise<void> {
    catalogLoading.value = true;
    error.value = null;
    try {
      const cacheKey = cacheKeyForQuery(query);
      const cached = searchCache.get(cacheKey);
      if (cached && Date.now() - cached.timestamp < CACHE_TTL_MS) {
        catalogEntries.value = cached.entries;
        catalogLoading.value = false;
        return;
      }
      catalogEntries.value = await unwrapCommandResult(commands.listSkillCatalog(query));
      searchCache.set(cacheKey, {
        entries: catalogEntries.value,
        timestamp: Date.now()
      });
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      catalogLoading.value = false;
    }
  }

  async function loadCatalogSources(): Promise<void> {
    catalogLoading.value = true;
    error.value = null;
    try {
      catalogSources.value = await unwrapCommandResult(commands.listSkillSources());
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      catalogLoading.value = false;
    }
  }

  async function addCatalogSource(config: SkillSourceView): Promise<void> {
    catalogLoading.value = true;
    error.value = null;
    try {
      await unwrapCommandResult(commands.addSkillSource(config));
      await loadCatalogSources();
    } catch (caughtError) {
      error.value = formatError(caughtError);
      throw caughtError;
    } finally {
      catalogLoading.value = false;
    }
  }

  async function removeCatalogSource(id: string): Promise<void> {
    catalogLoading.value = true;
    error.value = null;
    try {
      await unwrapCommandResult(commands.removeSkillSource(id));
      catalogSources.value = catalogSources.value.filter((s) => s.id !== id);
    } catch (caughtError) {
      error.value = formatError(caughtError);
      throw caughtError;
    } finally {
      catalogLoading.value = false;
    }
  }

  function isCatalogSourceEnabled(id: string): boolean {
    return catalogSources.value.some((s) => s.id === id && s.enabled);
  }

  async function toggleCatalogSource(id: string): Promise<void> {
    const source = catalogSources.value.find((s) => s.id === id);
    if (!source) return;
    await setCatalogSourceEnabled(id, !source.enabled);
  }

  async function setCatalogSourceEnabled(id: string, enabled: boolean): Promise<void> {
    error.value = null;
    try {
      await unwrapCommandResult(commands.setSkillSourceEnabled(id, enabled));
      catalogSources.value = catalogSources.value.map((s) => (s.id === id ? { ...s, enabled } : s));
    } catch (caughtError) {
      error.value = formatError(caughtError);
    }
  }

  async function refreshCatalog(): Promise<void> {
    catalogLoading.value = true;
    error.value = null;
    try {
      await unwrapCommandResult(commands.refreshSkillCatalog());
      searchCache.clear();
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      catalogLoading.value = false;
    }
  }

  return {
    skills,
    activeSkills,
    selectedSkill,
    loading,
    activatingSkillId,
    error,
    skillSettings,
    remoteResults,
    settingsLoading,
    remoteLoading,
    hasSkills,
    activeSkillIds,
    isSkillActive,
    loadSkills,
    loadSkillDetail,
    activateSkill,
    deactivateSkill,
    loadSkillSettings,
    setSkillEnabled,
    deleteSkill,
    searchRemoteSkills,
    installRemoteSkill,
    installGithubSkill,
    updateSkill,
    catalogEntries,
    catalogSources,
    catalogLoading,
    searchCatalog,
    loadCatalogSources,
    addCatalogSource,
    removeCatalogSource,
    isCatalogSourceEnabled,
    toggleCatalogSource,
    setCatalogSourceEnabled,
    refreshCatalog
  };
});

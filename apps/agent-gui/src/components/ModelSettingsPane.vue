<script setup lang="ts">
import type { ProfileSettingsView } from "@/generated/commands";
import { useNotifications } from "@/composables/useNotifications";
import ModelProfileCard from "@/components/ModelProfileCard.vue";
import ModelProfileFormDialog from "@/components/ModelProfileFormDialog.vue";
import SettingsCardList from "@/components/ui/SettingsCardList.vue";
import { storeToRefs } from "pinia";
import { useModelProfilesStore, formatError } from "@/stores/modelProfiles";

type ModelProfileSort = "original" | "alias" | "provider" | "source" | "status";

interface SortOption {
  value: ModelProfileSort;
  label: string;
}

const { t } = useI18n();
const { notify } = useNotifications();
const store = useModelProfilesStore();
const { profiles, loading, error, busyAlias } = storeToRefs(store);

const addDialogOpen = ref(false);
const editDialogOpen = ref(false);
const editingProfile = ref<ProfileSettingsView | null>(null);
const advancedOpen = ref(false);
const editAdvancedOpen = ref(false);
const formAlias = ref("");
const formProvider = ref("");
const formModelId = ref("");
const formContextWindow = ref("");
const formOutputLimit = ref("");
const formTemperature = ref("");
const formTopP = ref("");
const formTopK = ref("");
const formMaxTokens = ref("");
const formBaseUrl = ref("");
const formApiKeyEnv = ref("");
const searchQuery = ref("");
const profileSort = ref<ModelProfileSort>("original");
const sortCollator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

const sortOptions: SortOption[] = [
  { value: "original", label: "Original order" },
  { value: "alias", label: "Alias" },
  { value: "provider", label: "Provider" },
  { value: "source", label: "Source" },
  { value: "status", label: "Status" }
];

const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

watch(
  [() => configSource?.value, () => configProjectId?.value],
  () => {
    void store.loadProfiles(configSource?.value);
  },
  { immediate: true }
);

const normalizedSearchQuery = computed(() => searchQuery.value.trim().toLowerCase());

function sourceLabel(source: string): string {
  switch (source) {
    case "defaults":
      return t("models.sourceDefaults");
    case "profiles_toml":
      return t("models.sourceProfilesToml");
    case "user_config":
      return t("models.sourceUserConfig");
    case "project_config":
      return t("models.sourceProjectConfig");
    default:
      return source;
  }
}

function searchableProfileText(profile: ProfileSettingsView): string {
  return [
    profile.alias,
    profile.provider,
    profile.model_id,
    profile.source,
    sourceLabel(profile.source),
    profile.enabled ? t("models.enabled") : t("models.disabled"),
    profile.has_api_key ? t("models.hasApiKey") : t("models.noApiKey"),
    profile.base_url,
    profile.api_key_env,
    profile.config_path,
    profile.context_window?.toString(),
    profile.output_limit?.toString(),
    profile.temperature?.toString(),
    profile.top_p?.toString(),
    profile.top_k?.toString(),
    profile.max_tokens?.toString()
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
}

const filteredProfiles = computed(() => {
  const query = normalizedSearchQuery.value;
  if (!query) return profiles.value;
  return profiles.value.filter((profile) => searchableProfileText(profile).includes(query));
});

function profileSortValue(profile: ProfileSettingsView, sort: ModelProfileSort): string {
  switch (sort) {
    case "alias":
      return profile.alias;
    case "provider":
      return profile.provider;
    case "source":
      return sourceLabel(profile.source);
    case "status":
      return profile.enabled ? t("models.enabled") : t("models.disabled");
    case "original":
      return "";
  }
}

const visibleProfiles = computed(() => {
  if (profileSort.value === "original") return filteredProfiles.value;

  return filteredProfiles.value
    .map((profile, index) => ({ profile, index }))
    .sort((first, second) => {
      const comparison = sortCollator.compare(
        profileSortValue(first.profile, profileSort.value),
        profileSortValue(second.profile, profileSort.value)
      );
      return comparison || first.index - second.index;
    })
    .map(({ profile }) => profile);
});

function profileIndex(profile: ProfileSettingsView): number {
  return profiles.value.findIndex((item) => item.alias === profile.alias);
}

function resetForm(): void {
  formAlias.value = "";
  formProvider.value = "";
  formModelId.value = "";
  formContextWindow.value = "";
  formOutputLimit.value = "";
  formTemperature.value = "";
  formTopP.value = "";
  formTopK.value = "";
  formMaxTokens.value = "";
  formBaseUrl.value = "";
  formApiKeyEnv.value = "";
  advancedOpen.value = false;
  editAdvancedOpen.value = false;
}

function openAddDialog(): void {
  resetForm();
  addDialogOpen.value = true;
}

function closeAddDialog(): void {
  addDialogOpen.value = false;
  resetForm();
}

function openEditDialog(profile: ProfileSettingsView): void {
  editingProfile.value = profile;
  formAlias.value = profile.alias;
  formProvider.value = profile.provider;
  formModelId.value = profile.model_id;
  formContextWindow.value = profile.context_window?.toString() ?? "";
  formOutputLimit.value = profile.output_limit?.toString() ?? "";
  formTemperature.value = profile.temperature?.toString() ?? "";
  formTopP.value = profile.top_p?.toString() ?? "";
  formTopK.value = profile.top_k?.toString() ?? "";
  formMaxTokens.value = profile.max_tokens?.toString() ?? "";
  formBaseUrl.value = profile.base_url ?? "";
  formApiKeyEnv.value = profile.api_key_env ?? "";
  editAdvancedOpen.value = false;
  editDialogOpen.value = true;
}

function closeEditDialog(): void {
  editDialogOpen.value = false;
  editingProfile.value = null;
  resetForm();
}

function parseOptionalNumber(val: string): number | null {
  const trimmed = val.trim();
  if (!trimmed) return null;
  const num = Number(trimmed);
  return Number.isNaN(num) ? null : num;
}

function buildProfileInput(alias: string, enabled: boolean) {
  return {
    alias,
    provider: formProvider.value.trim(),
    model_id: formModelId.value.trim(),
    enabled,
    context_window: parseOptionalNumber(formContextWindow.value),
    output_limit: parseOptionalNumber(formOutputLimit.value),
    temperature: parseOptionalNumber(formTemperature.value),
    top_p: parseOptionalNumber(formTopP.value),
    top_k: parseOptionalNumber(formTopK.value)
      ? Math.trunc(parseOptionalNumber(formTopK.value)!)
      : null,
    max_tokens: parseOptionalNumber(formMaxTokens.value),
    base_url: formBaseUrl.value.trim() || null,
    api_key_env: formApiKeyEnv.value.trim() || null
  };
}

async function saveNewProfile(): Promise<void> {
  const alias = formAlias.value.trim();
  if (!alias || !formProvider.value.trim() || !formModelId.value.trim()) return;
  await store.upsertProfile(buildProfileInput(alias, true));
  if (!store.error) closeAddDialog();
}

async function saveEditProfile(): Promise<void> {
  const alias = formAlias.value.trim();
  if (!alias || !formProvider.value.trim() || !formModelId.value.trim()) return;
  await store.upsertProfile(buildProfileInput(alias, editingProfile.value?.enabled ?? true));
  if (!store.error) closeEditDialog();
}

async function testProfileConnectivity(profile: ProfileSettingsView): Promise<void> {
  try {
    const result = await store.testModelConnectivity(profile.alias);
    if (result.status === "ok" && result.data.ok === true) {
      notify("success", t("models.testSuccess", { alias: profile.alias }));
    } else {
      const msg =
        result.status === "error"
          ? String(result.error)
          : (result.data.error ?? t("models.testFailed", { alias: profile.alias }));
      notify("error", msg);
    }
  } catch (caughtError) {
    notify(
      "error",
      t("models.testFailed", { alias: profile.alias, error: formatError(caughtError) })
    );
  }
}

async function testFormConnectivity(): Promise<void> {
  const url = formBaseUrl.value.trim();
  if (!url) return;
  try {
    const result = await store.testUrlConnectivity(url);
    if (result.status === "ok" && result.data.ok === true) {
      notify("success", t("models.testSuccess", { alias: url }));
    } else {
      const msg =
        result.status === "error"
          ? String(result.error)
          : (result.data.error ?? t("models.testFailed", { alias: url }));
      notify("error", msg);
    }
  } catch (caughtError) {
    notify("error", t("models.testFailed", { alias: url, error: formatError(caughtError) }));
  }
}

function toggleProfile(profile: ProfileSettingsView): void {
  void store.setProfileEnabled(profile.alias, !profile.enabled);
}
</script>

<template>
  <section class="model-settings" :aria-label="t('models.title')" data-test="model-settings-pane">
    <SettingsState v-if="error" tone="error" data-test="model-page-error">
      {{ error }}
    </SettingsState>

    <SettingsToolbar :aria-label="t('models.title')">
      <KxToolbarAction
        data-test="model-open-config-file"
        :title="t('models.openConfigFile')"
        @click="store.openConfigFile()"
      >
        {{ t("models.openConfigFile") }}
      </KxToolbarAction>
      <KxToolbarAction :disabled="loading" data-test="model-refresh" @click="store.loadProfiles()">
        {{ loading ? t("common.loading") : t("common.refresh") }}
      </KxToolbarAction>
      <KxToolbarAction variant="primary" data-test="model-add-profile" @click="openAddDialog()">
        {{ t("models.addProfile") }}
      </KxToolbarAction>
    </SettingsToolbar>

    <SettingsState v-if="loading" tone="loading" data-test="model-loading-state">
      {{ t("models.loading") }}
    </SettingsState>
    <SettingsState v-else-if="profiles.length === 0" tone="empty" data-test="model-empty-state">
      {{ t("models.noProfiles") }}
    </SettingsState>

    <template v-else>
      <SettingsFilterBar aria-label="Search model profiles" data-test="model-filters">
        <div class="settings-filter-bar__row">
          <KxInput
            v-model="searchQuery"
            type="search"
            size="compact"
            aria-label="Search model profiles"
            placeholder="Search model profiles"
            data-test="model-search-input"
          />
          <KxSelect
            v-model="profileSort"
            size="compact"
            class="model-settings__sort-select"
            aria-label="Model profile sort"
            data-test="model-sort-select"
          >
            <option v-for="option in sortOptions" :key="option.value" :value="option.value">
              {{ option.label }}
            </option>
          </KxSelect>
        </div>
      </SettingsFilterBar>

      <SettingsState
        v-if="visibleProfiles.length === 0"
        tone="empty"
        data-test="model-filter-empty-state"
      >
        No model profiles match your search.
      </SettingsState>

      <SettingsCardList
        v-else
        :aria-label="t('models.title')"
        data-test="model-list"
        class="model-settings__list"
      >
        <ModelProfileCard
          v-for="profile in visibleProfiles"
          :key="profile.alias"
          :profile="profile"
          :index="profileIndex(profile)"
          :total="profiles.length"
          :busy-alias="busyAlias"
          @move="store.moveProfile"
          @edit="openEditDialog"
          @toggle="toggleProfile"
          @test="testProfileConnectivity"
          @remove="store.removeProfile"
        />
      </SettingsCardList>
    </template>

    <ModelProfileFormDialog
      :open="addDialogOpen"
      mode="add"
      :loading="loading"
      v-model:alias="formAlias"
      v-model:provider="formProvider"
      v-model:model-id="formModelId"
      v-model:context-window="formContextWindow"
      v-model:output-limit="formOutputLimit"
      v-model:temperature="formTemperature"
      v-model:top-p="formTopP"
      v-model:top-k="formTopK"
      v-model:max-tokens="formMaxTokens"
      v-model:base-url="formBaseUrl"
      v-model:api-key-env="formApiKeyEnv"
      v-model:advanced-open="advancedOpen"
      @close="closeAddDialog"
      @save="saveNewProfile"
      @test="testFormConnectivity"
    />

    <ModelProfileFormDialog
      :open="editDialogOpen"
      mode="edit"
      :loading="loading"
      :can-test="Boolean(editingProfile)"
      v-model:alias="formAlias"
      v-model:provider="formProvider"
      v-model:model-id="formModelId"
      v-model:context-window="formContextWindow"
      v-model:output-limit="formOutputLimit"
      v-model:temperature="formTemperature"
      v-model:top-p="formTopP"
      v-model:top-k="formTopK"
      v-model:max-tokens="formMaxTokens"
      v-model:base-url="formBaseUrl"
      v-model:api-key-env="formApiKeyEnv"
      v-model:advanced-open="editAdvancedOpen"
      @close="closeEditDialog"
      @save="saveEditProfile"
      @test="editingProfile && testProfileConnectivity(editingProfile)"
    />
  </section>
</template>

<style scoped>
.model-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow: hidden;
}

.model-settings__sort-select {
  flex: 0 0 160px;
}
</style>

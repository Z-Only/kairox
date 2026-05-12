<script setup lang="ts">
import type { ProfileSettingsView } from "@/generated/commands";
import { commands } from "@/generated/commands";

const { t } = useI18n();
const profiles = ref<ProfileSettingsView[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const busyAlias = ref<string | null>(null);
const addDialogOpen = ref(false);
const editDialogOpen = ref(false);
const editingProfile = ref<ProfileSettingsView | null>(null);
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

onMounted(() => {
  void fetchProfiles();
});

function formatError(caughtError: unknown): string {
  return caughtError instanceof Error ? caughtError.message : String(caughtError);
}

function isCommandResult<T>(result: T | { status: string }): result is { status: string } {
  return (
    typeof result === "object" &&
    result !== null &&
    "status" in result &&
    (result.status === "ok" || result.status === "error")
  );
}

async function unwrapCommandResult<T>(
  resultPromise: Promise<T | { status: string; data?: T; error?: string }>
): Promise<T> {
  const result = await resultPromise;
  if (!isCommandResult(result)) {
    return result;
  }
  if (result.status === "error") {
    throw new Error((result as { error?: string }).error);
  }
  return (result as { data: T }).data;
}

async function fetchProfiles(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    profiles.value = await unwrapCommandResult(commands.listProfileSettings());
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    loading.value = false;
  }
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

async function saveNewProfile(): Promise<void> {
  const alias = formAlias.value.trim();
  if (!alias || !formProvider.value.trim() || !formModelId.value.trim()) {
    return;
  }

  loading.value = true;
  error.value = null;
  try {
    await unwrapCommandResult(
      commands.upsertProfileSettings({
        alias,
        provider: formProvider.value.trim(),
        model_id: formModelId.value.trim(),
        enabled: true,
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
      })
    );
    closeAddDialog();
    await fetchProfiles();
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    loading.value = false;
  }
}

async function saveEditProfile(): Promise<void> {
  const alias = formAlias.value.trim();
  if (!alias || !formProvider.value.trim() || !formModelId.value.trim()) {
    return;
  }

  loading.value = true;
  error.value = null;
  try {
    await unwrapCommandResult(
      commands.upsertProfileSettings({
        alias,
        provider: formProvider.value.trim(),
        model_id: formModelId.value.trim(),
        enabled: editingProfile.value?.enabled ?? true,
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
      })
    );
    closeEditDialog();
    await fetchProfiles();
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    loading.value = false;
  }
}

async function toggleProfile(profile: ProfileSettingsView): Promise<void> {
  busyAlias.value = profile.alias;
  error.value = null;
  try {
    await unwrapCommandResult(commands.setProfileEnabled(profile.alias, !profile.enabled));
    await fetchProfiles();
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    busyAlias.value = null;
  }
}

async function deleteProfile(profile: ProfileSettingsView): Promise<void> {
  busyAlias.value = profile.alias;
  error.value = null;
  try {
    await unwrapCommandResult(commands.deleteProfileSettings(profile.alias));
    await fetchProfiles();
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    busyAlias.value = null;
  }
}

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
</script>

<template>
  <section class="model-settings" aria-label="Model settings" data-test="model-settings-pane">
    <p v-if="error" class="alert alert-error" role="alert" data-test="model-page-error">
      {{ error }}
    </p>

    <div class="mcp-toolbar">
      <button
        class="btn"
        type="button"
        :disabled="loading"
        data-test="model-refresh"
        @click="fetchProfiles()"
      >
        {{ loading ? t("common.loading") : t("common.refresh") }}
      </button>
      <button
        class="btn btn-primary"
        type="button"
        data-test="model-add-profile"
        @click="openAddDialog()"
      >
        {{ t("models.addProfile") }}
      </button>
    </div>

    <p v-if="loading" class="alert alert-info" role="status">
      {{ t("models.loading") }}
    </p>
    <p v-else-if="profiles.length === 0" class="empty-state">
      {{ t("models.noProfiles") }}
    </p>

    <div v-else class="model-settings__list" role="list" aria-label="Configured model profiles">
      <article
        v-for="profile in profiles"
        :key="profile.alias"
        class="card model-settings__profile"
        role="listitem"
        :data-test="`model-row-${profile.alias}`"
      >
        <div class="card-body model-settings__profile-body">
          <div class="model-settings__profile-main">
            <h3>{{ profile.alias }}</h3>
            <p>{{ profile.provider }} / {{ profile.model_id }}</p>
            <div class="mcp-settings__tags" aria-label="Profile metadata">
              <span :class="['tag', profile.enabled ? 'tag-success' : 'tag-warning']">
                {{ profile.enabled ? t("models.enabled") : t("models.disabled") }}
              </span>
              <span :class="['tag', profile.has_api_key ? 'tag-success' : 'tag-warning']">
                {{ profile.has_api_key ? t("models.hasApiKey") : t("models.noApiKey") }}
              </span>
              <span v-if="profile.context_window" class="tag">
                {{ t("models.contextWindow") }}: {{ profile.context_window.toLocaleString() }}
              </span>
              <span v-if="profile.output_limit" class="tag">
                {{ t("models.outputLimit") }}: {{ profile.output_limit.toLocaleString() }}
              </span>
              <span v-if="profile.temperature != null" class="tag">
                {{ t("models.temperature") }}: {{ profile.temperature }}
              </span>
              <span class="tag tag-muted">{{ sourceLabel(profile.source) }}</span>
            </div>
          </div>

          <div class="mcp-settings__actions" aria-label="Profile actions">
            <button
              class="btn btn-sm"
              type="button"
              :disabled="busyAlias === profile.alias"
              :data-test="`model-edit-${profile.alias}`"
              @click="openEditDialog(profile)"
            >
              {{ t("common.edit") }}
            </button>
            <button
              class="btn btn-sm"
              type="button"
              :disabled="busyAlias === profile.alias"
              :data-test="`model-enable-${profile.alias}`"
              @click="toggleProfile(profile)"
            >
              {{ profile.enabled ? t("models.disable") : t("models.enable") }}
            </button>
            <button
              v-if="profile.writable"
              class="btn btn-danger btn-sm"
              type="button"
              :disabled="busyAlias === profile.alias"
              :data-test="`model-delete-${profile.alias}`"
              @click="deleteProfile(profile)"
            >
              {{ t("common.delete") }}
            </button>
          </div>
        </div>
      </article>
    </div>

    <!-- Add Profile Dialog -->
    <ModalDialog
      :open="addDialogOpen"
      :title="t('models.addProfile')"
      :description="t('models.addProfileDesc')"
      data-test="model-add-dialog"
      @close="closeAddDialog"
    >
      <form class="mcp-settings__form" data-test="model-add-form" @submit.prevent="saveNewProfile">
        <label for="model-add-alias">{{ t("models.alias") }}</label>
        <input id="model-add-alias" v-model="formAlias" data-test="model-form-alias" required />

        <label for="model-add-provider">{{ t("models.provider") }}</label>
        <input
          id="model-add-provider"
          v-model="formProvider"
          data-test="model-form-provider"
          required
        />

        <label for="model-add-model-id">{{ t("models.modelId") }}</label>
        <input
          id="model-add-model-id"
          v-model="formModelId"
          data-test="model-form-model-id"
          required
        />

        <label for="model-add-base-url">{{ t("models.baseUrl") }}</label>
        <input id="model-add-base-url" v-model="formBaseUrl" data-test="model-form-base-url" />

        <label for="model-add-api-key-env">{{ t("models.apiKeyEnv") }}</label>
        <input
          id="model-add-api-key-env"
          v-model="formApiKeyEnv"
          data-test="model-form-api-key-env"
        />

        <label for="model-add-ctx">{{ t("models.contextWindow") }}</label>
        <input
          id="model-add-ctx"
          v-model="formContextWindow"
          type="number"
          data-test="model-form-ctx"
        />

        <label for="model-add-out">{{ t("models.outputLimit") }}</label>
        <input
          id="model-add-out"
          v-model="formOutputLimit"
          type="number"
          data-test="model-form-out"
        />

        <label for="model-add-temp">{{ t("models.temperature") }}</label>
        <input
          id="model-add-temp"
          v-model="formTemperature"
          type="number"
          step="0.1"
          min="0"
          max="2"
          data-test="model-form-temp"
        />

        <label for="model-add-top-p">{{ t("models.topP") }}</label>
        <input
          id="model-add-top-p"
          v-model="formTopP"
          type="number"
          step="0.1"
          min="0"
          max="1"
          data-test="model-form-top-p"
        />

        <label for="model-add-top-k">{{ t("models.topK") }}</label>
        <input
          id="model-add-top-k"
          v-model="formTopK"
          type="number"
          min="0"
          data-test="model-form-top-k"
        />

        <label for="model-add-max-tokens">{{ t("models.maxTokens") }}</label>
        <input
          id="model-add-max-tokens"
          v-model="formMaxTokens"
          type="number"
          data-test="model-form-max-tokens"
        />
      </form>

      <template #footer>
        <button class="btn" type="button" @click="closeAddDialog">
          {{ t("common.cancel") }}
        </button>
        <button
          class="btn btn-primary"
          type="submit"
          :disabled="loading || !formAlias.trim() || !formProvider.trim() || !formModelId.trim()"
          data-test="model-save-button"
          @click.prevent="saveNewProfile"
        >
          {{ loading ? t("models.saving") : t("models.saveProfile") }}
        </button>
      </template>
    </ModalDialog>

    <!-- Edit Profile Dialog -->
    <ModalDialog
      :open="editDialogOpen"
      :title="t('models.editProfile')"
      :description="t('models.editProfileDesc')"
      data-test="model-edit-dialog"
      @close="closeEditDialog"
    >
      <form
        class="mcp-settings__form"
        data-test="model-edit-form"
        @submit.prevent="saveEditProfile"
      >
        <label for="model-edit-alias">{{ t("models.alias") }}</label>
        <input id="model-edit-alias" v-model="formAlias" data-test="model-edit-alias" readonly />

        <label for="model-edit-provider">{{ t("models.provider") }}</label>
        <input
          id="model-edit-provider"
          v-model="formProvider"
          data-test="model-edit-provider"
          required
        />

        <label for="model-edit-model-id">{{ t("models.modelId") }}</label>
        <input
          id="model-edit-model-id"
          v-model="formModelId"
          data-test="model-edit-model-id"
          required
        />

        <label for="model-edit-base-url">{{ t("models.baseUrl") }}</label>
        <input id="model-edit-base-url" v-model="formBaseUrl" data-test="model-edit-base-url" />

        <label for="model-edit-api-key-env">{{ t("models.apiKeyEnv") }}</label>
        <input
          id="model-edit-api-key-env"
          v-model="formApiKeyEnv"
          data-test="model-edit-api-key-env"
        />

        <label for="model-edit-ctx">{{ t("models.contextWindow") }}</label>
        <input
          id="model-edit-ctx"
          v-model="formContextWindow"
          type="number"
          data-test="model-edit-ctx"
        />

        <label for="model-edit-out">{{ t("models.outputLimit") }}</label>
        <input
          id="model-edit-out"
          v-model="formOutputLimit"
          type="number"
          data-test="model-edit-out"
        />

        <label for="model-edit-temp">{{ t("models.temperature") }}</label>
        <input
          id="model-edit-temp"
          v-model="formTemperature"
          type="number"
          step="0.1"
          min="0"
          max="2"
          data-test="model-edit-temp"
        />

        <label for="model-edit-top-p">{{ t("models.topP") }}</label>
        <input
          id="model-edit-top-p"
          v-model="formTopP"
          type="number"
          step="0.1"
          min="0"
          max="1"
          data-test="model-edit-top-p"
        />

        <label for="model-edit-top-k">{{ t("models.topK") }}</label>
        <input
          id="model-edit-top-k"
          v-model="formTopK"
          type="number"
          min="0"
          data-test="model-edit-top-k"
        />

        <label for="model-edit-max-tokens">{{ t("models.maxTokens") }}</label>
        <input
          id="model-edit-max-tokens"
          v-model="formMaxTokens"
          type="number"
          data-test="model-edit-max-tokens"
        />
      </form>

      <template #footer>
        <button class="btn" type="button" @click="closeEditDialog">
          {{ t("common.cancel") }}
        </button>
        <button
          class="btn btn-primary"
          type="submit"
          :disabled="loading || !formProvider.trim() || !formModelId.trim()"
          data-test="model-edit-save-button"
          @click.prevent="saveEditProfile"
        >
          {{ loading ? t("models.saving") : t("models.saveProfile") }}
        </button>
      </template>
    </ModalDialog>
  </section>
</template>

<style scoped>
.model-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.model-settings__list {
  display: grid;
  gap: 12px;
}

.model-settings__profile-body {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  justify-content: space-between;
}

.model-settings__profile-main {
  min-width: 0;
  display: grid;
  gap: 8px;
}

.model-settings__profile h3 {
  margin: 0 0 4px;
}

.tag-muted {
  opacity: 0.65;
}
</style>

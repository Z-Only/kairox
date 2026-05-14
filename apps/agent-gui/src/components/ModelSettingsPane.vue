<script setup lang="ts">
import type { ProfileSettingsView } from "@/generated/commands";
import { commands } from "@/generated/commands";
import { useNotifications } from "@/composables/useNotifications";

const { t } = useI18n();
const { notify } = useNotifications();
const profiles = ref<ProfileSettingsView[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);
const busyAlias = ref<string | null>(null);
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

const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

watch(
  [() => configSource?.value, () => configProjectId?.value],
  () => {
    void fetchProfiles();
  },
  { immediate: true }
);

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
    const filter = configSource?.value === "project" ? "project" : null;
    profiles.value = await unwrapCommandResult(commands.listProfileSettings(filter));
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

async function testConnectivity(profile: ProfileSettingsView): Promise<void> {
  busyAlias.value = profile.alias;
  try {
    const result = await commands.testModelConnectivity(profile.alias);
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

async function testFormConnectivity(): Promise<void> {
  const url = formBaseUrl.value.trim();
  if (!url) return;
  try {
    const result = await commands.testUrlConnectivity(url);
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

async function openConfigFile(): Promise<void> {
  try {
    await commands.openProfilesConfigFile();
  } catch {
    // best-effort
  }
}

async function moveProfile(alias: string, direction: number): Promise<void> {
  busyAlias.value = alias;
  error.value = null;
  try {
    await unwrapCommandResult(commands.moveProfileInOrder(alias, direction));
    await fetchProfiles();
  } catch (caughtError) {
    error.value = formatError(caughtError);
  } finally {
    busyAlias.value = null;
  }
}
</script>

<template>
  <section class="model-settings" aria-label="Model settings" data-test="model-settings-pane">
    <p v-if="error" class="alert alert-error" role="alert" data-test="model-page-error">
      {{ error }}
    </p>

    <div class="model-toolbar">
      <div class="model-toolbar__actions">
        <button
          class="btn btn-sm"
          type="button"
          data-test="model-open-config-file"
          :title="t('models.openConfigFile')"
          @click="openConfigFile()"
        >
          {{ t("models.openConfigFile") }}
        </button>
        <button
          class="btn btn-sm"
          type="button"
          :disabled="loading"
          data-test="model-refresh"
          @click="fetchProfiles()"
        >
          {{ loading ? t("common.loading") : t("common.refresh") }}
        </button>
        <button
          class="btn btn-sm btn-primary"
          type="button"
          data-test="model-add-profile"
          @click="openAddDialog()"
        >
          {{ t("models.addProfile") }}
        </button>
      </div>
    </div>

    <p v-if="loading" class="alert alert-info" role="status">
      {{ t("models.loading") }}
    </p>
    <p v-else-if="profiles.length === 0" class="empty-state">
      {{ t("models.noProfiles") }}
    </p>

    <div v-else class="model-settings__list" role="list" aria-label="Configured model profiles">
      <article
        v-for="(profile, index) in profiles"
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
              <span v-if="profile.context_window" class="tag">
                {{ t("models.contextWindow") }}: {{ profile.context_window.toLocaleString() }}
              </span>
              <span v-if="profile.output_limit" class="tag">
                {{ t("models.outputLimit") }}: {{ profile.output_limit.toLocaleString() }}
              </span>
              <span v-if="profile.temperature != null" class="tag">
                {{ t("models.temperature") }}: {{ profile.temperature }}
              </span>
            </div>
          </div>

          <div class="model-settings__actions" aria-label="Profile actions">
            <div class="model-settings__reorder">
              <button
                class="btn btn-sm btn-icon"
                type="button"
                :disabled="busyAlias === profile.alias || index === 0"
                :data-test="`model-move-up-${profile.alias}`"
                :title="t('models.moveUp')"
                @click="moveProfile(profile.alias, -1)"
              >
                ▲
              </button>
              <button
                class="btn btn-sm btn-icon"
                type="button"
                :disabled="busyAlias === profile.alias || index === profiles.length - 1"
                :data-test="`model-move-down-${profile.alias}`"
                :title="t('models.moveDown')"
                @click="moveProfile(profile.alias, 1)"
              >
                ▼
              </button>
            </div>
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
              class="btn btn-sm"
              type="button"
              :disabled="busyAlias === profile.alias"
              :data-test="`model-test-${profile.alias}`"
              :title="t('models.testConnectivity')"
              @click="testConnectivity(profile)"
            >
              {{ t("models.testConnectivity") }}
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
      <form class="model-form" data-test="model-add-form" @submit.prevent="saveNewProfile">
        <fieldset class="model-form__section">
          <legend>{{ t("models.basicOptions") }}</legend>
          <div class="model-form__grid model-form__grid--2col">
            <label>
              <span>{{ t("models.alias") }} *</span>
              <input
                id="model-add-alias"
                v-model="formAlias"
                data-test="model-form-alias"
                required
              />
            </label>
            <label>
              <span>{{ t("models.provider") }} *</span>
              <input
                id="model-add-provider"
                v-model="formProvider"
                data-test="model-form-provider"
                required
              />
            </label>
          </div>
          <label>
            <span>{{ t("models.modelId") }} *</span>
            <input
              id="model-add-model-id"
              v-model="formModelId"
              data-test="model-form-model-id"
              required
            />
          </label>
        </fieldset>

        <fieldset class="model-form__section">
          <legend>{{ t("models.connectionOptions") }}</legend>
          <label>
            <span>{{ t("models.baseUrl") }}</span>
            <input id="model-add-base-url" v-model="formBaseUrl" data-test="model-form-base-url" />
          </label>
          <label>
            <span>{{ t("models.apiKeyEnv") }}</span>
            <input
              id="model-add-api-key-env"
              v-model="formApiKeyEnv"
              data-test="model-form-api-key-env"
            />
          </label>
        </fieldset>

        <fieldset class="model-form__section">
          <legend>
            <button type="button" class="model-form__toggle" @click="advancedOpen = !advancedOpen">
              {{ advancedOpen ? "▾" : "▸" }} {{ t("models.advancedOptions") }}
            </button>
          </legend>
          <div v-if="advancedOpen" class="model-form__grid model-form__grid--3col">
            <label>
              <span>{{ t("models.contextWindow") }}</span>
              <input
                id="model-add-ctx"
                v-model="formContextWindow"
                type="number"
                data-test="model-form-ctx"
              />
            </label>
            <label>
              <span>{{ t("models.outputLimit") }}</span>
              <input
                id="model-add-out"
                v-model="formOutputLimit"
                type="number"
                data-test="model-form-out"
              />
            </label>
            <label>
              <span>{{ t("models.temperature") }}</span>
              <input
                id="model-add-temp"
                v-model="formTemperature"
                type="number"
                step="0.1"
                min="0"
                max="2"
                data-test="model-form-temp"
              />
            </label>
            <label>
              <span>{{ t("models.topP") }}</span>
              <input
                id="model-add-top-p"
                v-model="formTopP"
                type="number"
                step="0.1"
                min="0"
                max="1"
                data-test="model-form-top-p"
              />
            </label>
            <label>
              <span>{{ t("models.topK") }}</span>
              <input
                id="model-add-top-k"
                v-model="formTopK"
                type="number"
                min="0"
                data-test="model-form-top-k"
              />
            </label>
            <label>
              <span>{{ t("models.maxTokens") }}</span>
              <input
                id="model-add-max-tokens"
                v-model="formMaxTokens"
                type="number"
                data-test="model-form-max-tokens"
              />
            </label>
          </div>
        </fieldset>
      </form>

      <template #footer>
        <button class="btn" type="button" @click="closeAddDialog">
          {{ t("common.cancel") }}
        </button>
        <button
          class="btn btn-sm"
          type="button"
          :disabled="!formBaseUrl.trim()"
          data-test="model-test-form-btn"
          @click="testFormConnectivity()"
        >
          {{ t("models.testConnectivity") }}
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
      <form class="model-form" data-test="model-edit-form" @submit.prevent="saveEditProfile">
        <fieldset class="model-form__section">
          <legend>{{ t("models.basicOptions") }}</legend>
          <div class="model-form__grid model-form__grid--2col">
            <label>
              <span>{{ t("models.alias") }}</span>
              <input
                id="model-edit-alias"
                v-model="formAlias"
                data-test="model-edit-alias"
                readonly
              />
            </label>
            <label>
              <span>{{ t("models.provider") }} *</span>
              <input
                id="model-edit-provider"
                v-model="formProvider"
                data-test="model-edit-provider"
                required
              />
            </label>
          </div>
          <label>
            <span>{{ t("models.modelId") }} *</span>
            <input
              id="model-edit-model-id"
              v-model="formModelId"
              data-test="model-edit-model-id"
              required
            />
          </label>
        </fieldset>

        <fieldset class="model-form__section">
          <legend>{{ t("models.connectionOptions") }}</legend>
          <label>
            <span>{{ t("models.baseUrl") }}</span>
            <input id="model-edit-base-url" v-model="formBaseUrl" data-test="model-edit-base-url" />
          </label>
          <label>
            <span>{{ t("models.apiKeyEnv") }}</span>
            <input
              id="model-edit-api-key-env"
              v-model="formApiKeyEnv"
              data-test="model-edit-api-key-env"
            />
          </label>
        </fieldset>

        <fieldset class="model-form__section">
          <legend>
            <button
              type="button"
              class="model-form__toggle"
              @click="editAdvancedOpen = !editAdvancedOpen"
            >
              {{ editAdvancedOpen ? "▾" : "▸" }} {{ t("models.advancedOptions") }}
            </button>
          </legend>
          <div v-if="editAdvancedOpen" class="model-form__grid model-form__grid--3col">
            <label>
              <span>{{ t("models.contextWindow") }}</span>
              <input
                id="model-edit-ctx"
                v-model="formContextWindow"
                type="number"
                data-test="model-edit-ctx"
              />
            </label>
            <label>
              <span>{{ t("models.outputLimit") }}</span>
              <input
                id="model-edit-out"
                v-model="formOutputLimit"
                type="number"
                data-test="model-edit-out"
              />
            </label>
            <label>
              <span>{{ t("models.temperature") }}</span>
              <input
                id="model-edit-temp"
                v-model="formTemperature"
                type="number"
                step="0.1"
                min="0"
                max="2"
                data-test="model-edit-temp"
              />
            </label>
            <label>
              <span>{{ t("models.topP") }}</span>
              <input
                id="model-edit-top-p"
                v-model="formTopP"
                type="number"
                step="0.1"
                min="0"
                max="1"
                data-test="model-edit-top-p"
              />
            </label>
            <label>
              <span>{{ t("models.topK") }}</span>
              <input
                id="model-edit-top-k"
                v-model="formTopK"
                type="number"
                min="0"
                data-test="model-edit-top-k"
              />
            </label>
            <label>
              <span>{{ t("models.maxTokens") }}</span>
              <input
                id="model-edit-max-tokens"
                v-model="formMaxTokens"
                type="number"
                data-test="model-edit-max-tokens"
              />
            </label>
          </div>
        </fieldset>
      </form>

      <template #footer>
        <button class="btn" type="button" @click="closeEditDialog">
          {{ t("common.cancel") }}
        </button>
        <button
          class="btn btn-sm"
          type="button"
          :disabled="!editingProfile"
          data-test="model-edit-test-btn"
          @click="editingProfile && testConnectivity(editingProfile)"
        >
          {{ t("models.testConnectivity") }}
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
  overflow: hidden;
}

.model-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex-wrap: wrap;
  flex: none;
}

.model-toolbar__actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.model-settings__list {
  display: grid;
  gap: 12px;
  flex: 1;
  overflow-y: auto;
  min-height: 0;
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
  flex: 1;
}

.model-settings__profile h3 {
  margin: 0 0 4px;
}

.model-settings__actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.model-settings__reorder {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-right: 4px;
}

.btn-icon {
  padding: 2px 6px;
  line-height: 1;
  font-size: 0.7rem;
}

/* Form styles */
.model-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.model-form__section {
  border: none;
  padding: 0;
  margin: 0;
}

.model-form__section legend {
  font-weight: 600;
  font-size: 0.9rem;
  margin-bottom: 8px;
  color: var(--app-text-color-2);
  width: 100%;
}

.model-form__toggle {
  all: unset;
  cursor: pointer;
  font-weight: 600;
  font-size: 0.9rem;
  color: var(--app-text-color-2);
}

.model-form__toggle:hover {
  color: var(--color-text);
}

.model-form__toggle:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
  border-radius: 2px;
}

.model-form__grid {
  display: grid;
  gap: 8px;
}

.model-form__grid--2col {
  grid-template-columns: 1fr 1fr;
}

.model-form__grid--3col {
  grid-template-columns: 1fr 1fr 1fr;
}

.model-form label {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.model-form label > span {
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--app-text-color-2);
}

.model-form input {
  padding: 6px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  background: var(--app-card-color);
  color: var(--app-text-color);
  font-size: 0.85rem;
}

.model-form input:focus {
  border-color: var(--app-primary-color);
  outline: none;
}

.model-form input:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}

.model-settings button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>

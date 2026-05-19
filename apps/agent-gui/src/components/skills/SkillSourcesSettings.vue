<script setup lang="ts">
import { useSkillsStore } from "@/stores/skills";
import type { SkillSourceView } from "@/generated/commands";

const { t } = useI18n();
const store = useSkillsStore();
const showAddForm = ref(false);
const formError = ref<string | null>(null);

const draft = ref({
  id: "",
  display_name: "",
  kind: "skillhub",
  url: "",
  search_template:
    "/api/skills?keyword={{query}}&page=1&pageSize={{limit}}&sortBy=downloads&order=desc",
  download_template: "/api/v1/download?slug={{slug}}",
  list_template: "",
  detail_template: "/api/v1/skills/{{slug}}",
  enabled: true,
  priority: 100,
  cache_ttl_seconds: 900
});

const kindOptions = [{ label: "SkillHub", value: "skillhub" }];

onMounted(() => {
  void store.loadCatalogSources();
});

function isValidUrl(u: string): boolean {
  return u.startsWith("http://") || u.startsWith("https://");
}

function resetDraft(): void {
  draft.value = {
    id: "",
    display_name: "",
    kind: "skillhub",
    url: "",
    search_template:
      "/api/skills?keyword={{query}}&page=1&pageSize={{limit}}&sortBy=downloads&order=desc",
    download_template: "/api/v1/download?slug={{slug}}",
    list_template: "",
    detail_template: "/api/v1/skills/{{slug}}",
    enabled: true,
    priority: 100,
    cache_ttl_seconds: 900
  };
}

async function save(): Promise<void> {
  formError.value = null;
  if (!draft.value.id.trim() || !draft.value.display_name.trim()) {
    formError.value = t("skills.sourceFormError.idAndDisplayNameRequired");
    return;
  }
  if (!isValidUrl(draft.value.url)) {
    formError.value = t("skills.sourceFormError.urlMustStartWithHttp");
    return;
  }
  if (!draft.value.search_template.trim() || !draft.value.download_template.trim()) {
    formError.value = t("skills.sourceFormError.searchAndDownloadRequired");
    return;
  }
  const config: SkillSourceView = {
    id: draft.value.id.trim(),
    display_name: draft.value.display_name.trim(),
    kind: draft.value.kind,
    url: draft.value.url,
    search_template: draft.value.search_template.trim(),
    download_template: draft.value.download_template.trim(),
    list_template: draft.value.list_template.trim() || null,
    detail_template: draft.value.detail_template.trim() || null,
    field_mapping: {
      name_path: "name",
      description_path: "description",
      install_count_path: null,
      github_stars_path: null,
      package_path: "id",
      source_url_path: null
    },
    enabled: draft.value.enabled,
    priority: draft.value.priority,
    cache_ttl_seconds: draft.value.cache_ttl_seconds,
    last_error: null
  };
  try {
    await store.addCatalogSource(config);
    showAddForm.value = false;
    resetDraft();
  } catch (err) {
    formError.value = formatError(err);
  }
}

async function onToggle(id: string, enabled: boolean): Promise<void> {
  await store.setCatalogSourceEnabled(id, enabled);
}

async function onRemove(id: string): Promise<void> {
  await store.removeCatalogSource(id);
}

function formatError(caughtError: unknown): string {
  return caughtError instanceof Error ? caughtError.message : String(caughtError);
}
</script>

<template>
  <div class="skill-sources-settings">
    <h3 class="header">
      <strong>{{ t("skills.catalogSourcesTitle") }}</strong>
    </h3>

    <SettingsState
      v-if="store.catalogSources.length === 0"
      tone="empty"
      data-test="skill-sources-empty-state"
    >
      {{ t("skills.sourcesEmpty") }}
    </SettingsState>

    <ul v-else class="sources-list">
      <li v-for="src in store.catalogSources" :key="src.id" class="src-row">
        <div class="src-meta">
          <div class="src-meta-row">
            <strong>{{ src.display_name }}</strong>
            <code class="src-id">{{ src.id }}</code>
            <span class="tag tag-info src-kind">{{ src.kind }}</span>
          </div>
          <a
            v-if="src.url"
            :href="src.url"
            target="_blank"
            rel="noopener noreferrer"
            class="src-url"
          >
            {{ src.url }}
          </a>
          <span
            v-if="src.last_error"
            class="src-error text-error"
            :title="t('skills.sourceErrorTitle', { error: src.last_error })"
          >
            {{ t("skills.sourceError", { error: src.last_error }) }}
          </span>
        </div>
        <div class="src-actions">
          <label class="src-enable" :data-test="`skill-src-enable-${src.id}`">
            <input
              type="checkbox"
              :checked="src.enabled"
              :disabled="src.id === 'skillhub'"
              @change="onToggle(src.id, ($event.target as HTMLInputElement).checked)"
            />
            {{ t("skills.sourceEnabled") }}
          </label>
          <button
            v-if="src.id !== 'skillhub'"
            class="btn btn-error-ghost"
            :data-test="`skill-src-remove-${src.id}`"
            @click="onRemove(src.id)"
          >
            {{ t("common.delete") }}
          </button>
        </div>
      </li>
    </ul>

    <button
      v-if="!showAddForm"
      class="btn"
      type="button"
      data-test="skill-add-source-toggle"
      @click="showAddForm = true"
    >
      {{ t("skills.addSource") }}
    </button>

    <div v-else class="add-form">
      <KxFormField label="id">
        <KxInput v-model="draft.id" data-test="skill-src-id" />
      </KxFormField>
      <KxFormField :label="t('skills.displayName')">
        <KxInput v-model="draft.display_name" data-test="skill-src-name" />
      </KxFormField>
      <KxFormField :label="t('skills.kind')">
        <KxSelect v-model="draft.kind">
          <option v-for="opt in kindOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </option>
        </KxSelect>
      </KxFormField>
      <KxFormField :label="t('skills.url')">
        <KxInput v-model="draft.url" data-test="skill-src-url" />
      </KxFormField>
      <KxFormField :label="t('skills.searchTemplate')" required>
        <KxInput v-model="draft.search_template" data-test="skill-src-search-template" />
      </KxFormField>
      <KxFormField :label="t('skills.downloadTemplate')" required>
        <KxInput v-model="draft.download_template" data-test="skill-src-download-template" />
      </KxFormField>
      <KxFormField :label="t('skills.listTemplate')">
        <KxInput v-model="draft.list_template" data-test="skill-src-list-template" />
      </KxFormField>
      <KxFormField :label="t('skills.detailTemplate')">
        <KxInput v-model="draft.detail_template" data-test="skill-src-detail-template" />
      </KxFormField>
      <span v-if="formError" class="error text-error">
        {{ formError }}
      </span>
      <KxFormActions>
        <button class="btn btn-primary" type="button" data-test="skill-src-save" @click="save">
          {{ t("common.save") }}
        </button>
        <button
          class="btn"
          type="button"
          @click="
            showAddForm = false;
            formError = null;
          "
        >
          {{ t("common.cancel") }}
        </button>
      </KxFormActions>
    </div>
  </div>
</template>

<style scoped>
.skill-sources-settings {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.header {
  font-size: 14px;
  margin: 0;
  font-weight: normal;
}

.sources-list {
  list-style: none;
  margin: 0;
  padding: 0;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
}

.src-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color);
}

.src-row:last-child {
  border-bottom: none;
}

.src-row:hover {
  background: var(--app-hover-color);
}

.src-meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.src-meta-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.src-id {
  font-size: 0.85em;
  color: var(--app-text-color-2);
}

.tag {
  display: inline-block;
  padding: 0 6px;
  border-radius: 3px;
  font-size: 0.75em;
  line-height: 1.8;
}

.tag-info {
  background: var(--app-code-bg);
  color: var(--app-info-color);
}

.src-kind {
  text-transform: uppercase;
}

.src-url {
  font-size: 0.85em;
  color: var(--app-text-color-3);
  text-decoration: none;
}

.src-url:hover {
  color: var(--app-primary-color);
  text-decoration: underline;
}

.src-error {
  font-size: 0.85em;
}

.text-error {
  color: var(--app-error-color);
}

.src-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.src-enable {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 0.85em;
  cursor: pointer;
  color: var(--app-text-color);
}

.add-form {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border: 1px dashed var(--app-border-color);
  border-radius: 4px;
}

.error {
  margin: 0;
}
</style>

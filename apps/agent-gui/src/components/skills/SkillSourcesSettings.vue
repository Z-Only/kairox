<script setup lang="ts">
import { useSkillsStore } from "@/stores/skills";
import type { SkillSourceView } from "@/generated/commands";
import SettingsItemSummary from "@/components/ui/SettingsItemSummary.vue";
import SettingsStatusTag from "@/components/ui/SettingsStatusTag.vue";

const { t } = useI18n();
const store = useSkillsStore();
const showAddForm = ref(false);
const formError = ref<string | null>(null);
const sourceSearchQuery = ref("");
const templateTokens = {
  query: "{{query}}",
  limit: "{{limit}}",
  slug: "{{slug}}"
};

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

const kindOptions = computed(() => [{ label: t("skills.sourceKindSkillHub"), value: "skillhub" }]);
const normalizedSourceSearchQuery = computed(() => sourceSearchQuery.value.trim().toLowerCase());
const filteredCatalogSources = computed(() => {
  const query = normalizedSourceSearchQuery.value;
  if (!query) return store.catalogSources;
  return store.catalogSources.filter((source) => searchableSourceText(source).includes(query));
});

onMounted(() => {
  void store.loadCatalogSources();
});

function isValidUrl(u: string): boolean {
  return u.startsWith("http://") || u.startsWith("https://");
}

function searchableSourceText(source: SkillSourceView): string {
  return [
    source.id,
    source.display_name,
    source.kind,
    source.url,
    source.search_template,
    source.download_template,
    source.list_template,
    source.detail_template,
    source.enabled ? "enabled" : "disabled",
    source.last_error
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
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

    <SettingsFilterBar
      v-if="store.catalogSources.length > 0"
      :aria-label="t('skills.catalogSourcesAria')"
      data-test="skill-source-filter-bar"
    >
      <form role="search" @submit.prevent>
        <KxInput
          v-model="sourceSearchQuery"
          type="search"
          :placeholder="t('skills.sourceSearchPlaceholder')"
          data-test="skill-source-search-input"
          size="compact"
        />
      </form>
    </SettingsFilterBar>

    <SettingsState
      v-if="store.catalogSources.length === 0"
      tone="empty"
      data-test="skill-sources-empty-state"
    >
      {{ t("skills.sourcesEmpty") }}
    </SettingsState>
    <SettingsState
      v-else-if="filteredCatalogSources.length === 0"
      tone="empty"
      data-test="skill-sources-filter-empty"
    >
      {{ t("skills.sourcesFilterEmpty") }}
    </SettingsState>

    <SettingsCardList
      v-else
      :aria-label="t('skills.catalogSourcesAria')"
      data-test="skill-sources-list"
      :scroll="false"
      dense
    >
      <SettingsCardItem
        v-for="src in filteredCatalogSources"
        :key="src.id"
        :data-test="`skill-source-row-${src.id}`"
      >
        <SettingsItemSummary
          :title="src.display_name"
          :heading-level="4"
          :tags-label="t('skills.allSources')"
        >
          <template #tags>
            <code>{{ src.id }}</code>
            <SettingsStatusTag tone="info" class="src-kind">{{ src.kind }}</SettingsStatusTag>
          </template>
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
        </SettingsItemSummary>

        <template #actions>
          <label class="src-enable" :data-test="`skill-src-enable-${src.id}`">
            <input
              type="checkbox"
              :checked="src.enabled"
              :disabled="src.id === 'skillhub'"
              @change="onToggle(src.id, ($event.target as HTMLInputElement).checked)"
            />
            {{ t("skills.sourceEnabled") }}
          </label>
          <KxButton
            v-if="src.id !== 'skillhub'"
            variant="danger-ghost"
            size="sm"
            :data-test="`skill-src-remove-${src.id}`"
            @click="onRemove(src.id)"
          >
            {{ t("common.delete") }}
          </KxButton>
        </template>
      </SettingsCardItem>
    </SettingsCardList>

    <KxButton v-if="!showAddForm" data-test="skill-add-source-toggle" @click="showAddForm = true">
      {{ t("skills.addSource") }}
    </KxButton>

    <div v-else class="add-form">
      <KxFormField :label="t('skills.sourceId')">
        <KxInput
          v-model="draft.id"
          data-test="skill-src-id"
          :placeholder="t('skills.sourceIdPlaceholder')"
        />
      </KxFormField>
      <KxFormField :label="t('skills.displayName')">
        <KxInput
          v-model="draft.display_name"
          data-test="skill-src-name"
          :placeholder="t('skills.displayNamePlaceholder')"
        />
      </KxFormField>
      <KxFormField :label="t('skills.kind')">
        <KxSelect v-model="draft.kind">
          <option v-for="opt in kindOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </option>
        </KxSelect>
      </KxFormField>
      <KxFormField :label="t('skills.url')">
        <KxInput
          v-model="draft.url"
          data-test="skill-src-url"
          :placeholder="t('skills.urlPlaceholder')"
        />
      </KxFormField>
      <KxFormField
        :label="t('skills.searchTemplate')"
        :description="t('skills.searchTemplateDescription', templateTokens)"
        required
      >
        <KxInput
          v-model="draft.search_template"
          data-test="skill-src-search-template"
          :placeholder="t('skills.searchTemplatePlaceholder', templateTokens)"
        />
      </KxFormField>
      <KxFormField
        :label="t('skills.downloadTemplate')"
        :description="t('skills.downloadTemplateDescription', templateTokens)"
        required
      >
        <KxInput
          v-model="draft.download_template"
          data-test="skill-src-download-template"
          :placeholder="t('skills.downloadTemplatePlaceholder', templateTokens)"
        />
      </KxFormField>
      <KxFormField :label="t('skills.listTemplate')">
        <KxInput
          v-model="draft.list_template"
          data-test="skill-src-list-template"
          :placeholder="t('skills.listTemplatePlaceholder')"
        />
      </KxFormField>
      <KxFormField :label="t('skills.detailTemplate')">
        <KxInput
          v-model="draft.detail_template"
          data-test="skill-src-detail-template"
          :placeholder="t('skills.detailTemplatePlaceholder', templateTokens)"
        />
      </KxFormField>
      <span v-if="formError" class="error text-error">
        {{ formError }}
      </span>
      <KxFormActions>
        <KxButton variant="primary" data-test="skill-src-save" @click="save">
          {{ t("common.save") }}
        </KxButton>
        <KxButton
          @click="
            showAddForm = false;
            formError = null;
          "
        >
          {{ t("common.cancel") }}
        </KxButton>
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

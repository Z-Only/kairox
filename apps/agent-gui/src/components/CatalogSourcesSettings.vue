<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useCatalogStore } from "@/stores/catalog";
import type {
  AddCatalogSourceRequestPayload,
  CatalogSourceViewResponse
} from "../generated/commands";
import SettingsItemSummary from "@/components/ui/SettingsItemSummary.vue";
import SettingsStatusTag from "@/components/ui/SettingsStatusTag.vue";

const { t } = useI18n();
const catalog = useCatalogStore();
const showAddForm = ref(false);
const formError = ref<string | null>(null);
const sourceSearchQuery = ref("");
const sourceSortMode = ref<CatalogSourceSortMode>("original");

type CatalogSourceSortMode = "original" | "name" | "priority" | "status" | "trust";

const draft = ref<AddCatalogSourceRequestPayload>({
  id: "",
  display_name: "",
  kind: "mcp_registry",
  url: "",
  api_key_env: null,
  priority: 100,
  default_trust: "community",
  enabled: true,
  cache_ttl_seconds: null
});

const sources = computed(() => catalog.sources);
const failures = computed(() => catalog.sourceFailures);
const normalizedSourceSearchQuery = computed(() => sourceSearchQuery.value.trim().toLowerCase());
const filteredSources = computed(() => {
  const query = normalizedSourceSearchQuery.value;
  if (!query) return sources.value;
  return sources.value.filter((source) => searchableSourceText(source).includes(query));
});
const displayedSources = computed(() => sortedSources(filteredSources.value, sourceSortMode.value));

const kindOptions = computed(() => [
  { label: t("marketplace.sourceKindMcpRegistry"), value: "mcp_registry" }
]);
const sourceSortOptions: { label: string; value: CatalogSourceSortMode }[] = [
  { label: "Original order", value: "original" },
  { label: "Name", value: "name" },
  { label: "Priority", value: "priority" },
  { label: "Status", value: "status" },
  { label: "Trust", value: "trust" }
];
const sourceNameCollator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
const sourceTrustOrder: Record<string, number> = {
  verified: 0,
  community: 1,
  unverified: 2
};

onMounted(() => {
  void catalog.fetchSources();
});

function isValidUrl(u: string): boolean {
  return u.startsWith("http://") || u.startsWith("https://");
}

function searchableSourceText(source: CatalogSourceViewResponse): string {
  return [
    source.id,
    source.display_name,
    source.kind,
    source.url,
    source.api_key_env,
    source.default_trust,
    source.enabled ? "enabled" : "disabled",
    source.last_error,
    failures.value[source.id]
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
}

function sortedSources(
  sourceList: CatalogSourceViewResponse[],
  sortMode: CatalogSourceSortMode
): CatalogSourceViewResponse[] {
  if (sortMode === "original") return sourceList;
  return sourceList
    .map((source, index) => ({ source, index }))
    .sort((left, right) => {
      const result = compareSources(left.source, right.source, sortMode);
      return result === 0 ? left.index - right.index : result;
    })
    .map(({ source }) => source);
}

function compareSources(
  left: CatalogSourceViewResponse,
  right: CatalogSourceViewResponse,
  sortMode: Exclude<CatalogSourceSortMode, "original">
): number {
  switch (sortMode) {
    case "name":
      return sourceNameCollator.compare(
        left.display_name || left.id,
        right.display_name || right.id
      );
    case "priority":
      return left.priority - right.priority;
    case "status":
      return Number(right.enabled) - Number(left.enabled);
    case "trust":
      return sourceTrustRank(left.default_trust) - sourceTrustRank(right.default_trust);
  }
}

function sourceTrustRank(trust: string): number {
  return sourceTrustOrder[trust] ?? Number.MAX_SAFE_INTEGER;
}

function resetDraft(): void {
  draft.value = {
    id: "",
    display_name: "",
    kind: "mcp_registry",
    url: "",
    api_key_env: null,
    priority: 100,
    default_trust: "community",
    enabled: true,
    cache_ttl_seconds: null
  };
}

async function save(): Promise<void> {
  formError.value = null;
  if (!draft.value.id || !draft.value.display_name) {
    formError.value = t("marketplace.sourceFormError.idAndDisplayNameRequired");
    return;
  }
  if (!isValidUrl(draft.value.url)) {
    formError.value = t("marketplace.sourceFormError.urlMustStartWithHttp");
    return;
  }
  await catalog.addSource({ ...draft.value });
  showAddForm.value = false;
  resetDraft();
}

function onToggleChecked(id: string, checked: boolean): void {
  void onToggle(id, checked);
}

async function onRemove(id: string): Promise<void> {
  if (id === "builtin") return;
  await catalog.removeSource(id);
}

async function onToggle(id: string, enabled: boolean): Promise<void> {
  await catalog.setSourceEnabled(id, enabled);
  if (enabled && id !== "builtin") {
    await catalog.refreshCatalogSource(id);
  } else {
    await catalog.fetchCatalog();
  }
}
</script>

<template>
  <div class="catalog-sources-settings">
    <h3 class="header">
      <strong>{{ t("marketplace.catalogSourcesTitle") }}</strong>
    </h3>

    <SettingsFilterBar
      v-if="sources.length > 0"
      :aria-label="t('marketplace.catalogSourcesAria')"
      data-test="catalog-source-filter-bar"
    >
      <div class="settings-filter-bar__row">
        <form class="source-search-form" role="search" @submit.prevent>
          <KxInput
            v-model="sourceSearchQuery"
            type="search"
            :placeholder="t('marketplace.sourceSearchPlaceholder')"
            data-test="catalog-source-search-input"
            size="compact"
          />
        </form>
        <KxSelect
          v-model="sourceSortMode"
          aria-label="Catalog source sort"
          data-test="catalog-source-sort-select"
          class="source-sort-select"
          size="compact"
        >
          <option v-for="opt in sourceSortOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </option>
        </KxSelect>
      </div>
    </SettingsFilterBar>

    <SettingsState v-if="sources.length === 0" tone="empty" data-test="catalog-sources-empty-state">
      {{ t("marketplace.sourcesEmpty") }}
    </SettingsState>
    <SettingsState
      v-else-if="filteredSources.length === 0"
      tone="empty"
      data-test="catalog-sources-filter-empty"
    >
      {{ t("marketplace.sourcesFilterEmpty") }}
    </SettingsState>

    <SettingsCardList
      v-else
      :aria-label="t('marketplace.catalogSourcesAria')"
      data-test="catalog-sources-list"
      :scroll="false"
      dense
    >
      <SettingsCardItem
        v-for="src in displayedSources"
        :key="src.id"
        :data-test="`catalog-source-row-${src.id}`"
      >
        <SettingsItemSummary
          :title="src.display_name"
          :heading-level="4"
          :tags-label="t('marketplace.sourceSettingsAria')"
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
            v-if="failures[src.id]"
            class="src-error text-error"
            :title="t('marketplace.sourceErrorTitle', { error: failures[src.id] })"
          >
            {{ t("marketplace.sourceError", { error: failures[src.id] }) }}
          </span>
        </SettingsItemSummary>

        <template #actions>
          <label class="src-enable" :data-test="`src-enable-${src.id}`">
            <input
              type="checkbox"
              :checked="src.enabled"
              :disabled="src.id === 'builtin'"
              @change="onToggleChecked(src.id, ($event.target as HTMLInputElement).checked)"
            />
            {{ t("marketplace.sourceEnabled") }}
          </label>
          <KxButton
            v-if="src.id !== 'builtin'"
            variant="danger-ghost"
            size="sm"
            :data-test="`src-remove-${src.id}`"
            @click="onRemove(src.id)"
          >
            {{ t("common.delete") }}
          </KxButton>
        </template>
      </SettingsCardItem>
    </SettingsCardList>

    <KxButton v-if="!showAddForm" data-test="add-source-toggle" @click="showAddForm = true">
      {{ t("marketplace.addSource") }}
    </KxButton>

    <div v-else class="add-form">
      <KxFormField :label="t('marketplace.sourceId')">
        <KxInput
          v-model="draft.id"
          data-test="src-id"
          :placeholder="t('marketplace.sourceIdPlaceholder')"
        />
      </KxFormField>
      <KxFormField :label="t('marketplace.displayName')">
        <KxInput
          v-model="draft.display_name"
          data-test="src-name"
          :placeholder="t('marketplace.displayNamePlaceholder')"
        />
      </KxFormField>
      <KxFormField :label="t('marketplace.kind')">
        <KxSelect v-model="draft.kind">
          <option v-for="opt in kindOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </option>
        </KxSelect>
      </KxFormField>
      <KxFormField :label="t('marketplace.url')">
        <KxInput
          v-model="draft.url"
          data-test="src-url"
          :placeholder="t('marketplace.urlPlaceholder')"
        />
      </KxFormField>
      <KxFormField
        :label="t('marketplace.apiKeyEnv')"
        :description="t('marketplace.apiKeyEnvDescription')"
      >
        <KxInput v-model="draft.api_key_env" :placeholder="t('marketplace.apiKeyEnvPlaceholder')" />
      </KxFormField>
      <span v-if="formError" class="error text-error">
        {{ formError }}
      </span>
      <KxFormActions>
        <KxButton variant="primary" data-test="src-save" @click="save">
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
.catalog-sources-settings {
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
.source-search-form {
  flex: 1 1 220px;
}
.source-sort-select {
  flex: 0 1 160px;
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

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useCatalogStore } from "@/stores/catalog";
import type { AddCatalogSourceRequestPayload } from "../generated/commands";

const { t } = useI18n();
const catalog = useCatalogStore();
const showAddForm = ref(false);
const formError = ref<string | null>(null);

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

const kindOptions: { label: string; value: string }[] = [
  { label: "MCP Registry", value: "mcp_registry" }
];

onMounted(() => {
  void catalog.fetchSources();
});

function isValidUrl(u: string): boolean {
  return u.startsWith("http://") || u.startsWith("https://");
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

    <KxStateBlock
      v-if="sources.length === 0"
      tone="empty"
      compact
      data-test="catalog-sources-empty-state"
    >
      {{ t("marketplace.sourcesEmpty") }}
    </KxStateBlock>

    <ul v-else class="list src-list">
      <li v-for="src in sources" :key="src.id" class="src-row">
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
            v-if="failures[src.id]"
            class="src-error text-error"
            :title="t('marketplace.sourceErrorTitle', { error: failures[src.id] })"
          >
            {{ t("marketplace.sourceError", { error: failures[src.id] }) }}
          </span>
        </div>
        <div class="src-actions">
          <label class="src-enable" :data-test="`src-enable-${src.id}`">
            <input
              type="checkbox"
              :checked="src.enabled"
              :disabled="src.id === 'builtin'"
              @change="onToggleChecked(src.id, ($event.target as HTMLInputElement).checked)"
            />
            {{ t("marketplace.sourceEnabled") }}
          </label>
          <button
            v-if="src.id !== 'builtin'"
            class="btn btn-error-ghost"
            :data-test="`src-remove-${src.id}`"
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
      data-test="add-source-toggle"
      @click="showAddForm = true"
    >
      {{ t("marketplace.addSource") }}
    </button>

    <div v-else class="add-form">
      <label class="field">
        <span class="field-label">id</span>
        <input v-model="draft.id" class="input" data-test="src-id" />
      </label>
      <label class="field">
        <span class="field-label">{{ t("marketplace.displayName") }}</span>
        <input v-model="draft.display_name" class="input" data-test="src-name" />
      </label>
      <label class="field">
        <span class="field-label">{{ t("marketplace.kind") }}</span>
        <select
          :value="draft.kind"
          class="input"
          @change="draft.kind = ($event.target as HTMLSelectElement).value"
        >
          <option v-for="opt in kindOptions" :key="opt.value" :value="opt.value">
            {{ opt.label }}
          </option>
        </select>
      </label>
      <label class="field">
        <span class="field-label">{{ t("marketplace.url") }}</span>
        <input v-model="draft.url" class="input" data-test="src-url" />
      </label>
      <label class="field">
        <span class="field-label">{{ t("marketplace.apiKeyEnv") }}</span>
        <input v-model="draft.api_key_env" class="input" />
      </label>
      <span v-if="formError" class="error text-error">
        {{ formError }}
      </span>
      <div class="form-actions">
        <button class="btn btn-primary" data-test="src-save" @click="save">
          {{ t("common.save") }}
        </button>
        <button
          class="btn"
          @click="
            showAddForm = false;
            formError = null;
          "
        >
          {{ t("common.cancel") }}
        </button>
      </div>
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
.empty-state {
  font-style: italic;
  color: var(--app-text-color-3);
  text-align: center;
  padding: 24px 0;
}
.list {
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
.btn {
  padding: 4px 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  background: var(--app-card-color);
  cursor: pointer;
  font-size: 0.85em;
  color: var(--app-text-color);
}
.btn-primary {
  background: var(--app-primary-color);
  color: #fff;
  border-color: var(--app-primary-color);
}
.btn-error-ghost {
  color: var(--app-error-color);
  border-color: var(--app-error-color);
  background: transparent;
}
.add-form {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border: 1px dashed var(--app-border-color);
  border-radius: 4px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.field-label {
  color: var(--app-text-color-2);
  font-size: 0.85em;
}
.input {
  padding: 4px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  font-size: 0.85em;
  background: var(--app-body-color);
  color: var(--app-text-color);
}
.error {
  margin: 0;
}
.form-actions {
  display: flex;
  gap: 8px;
  margin-top: 4px;
}
</style>

<script setup lang="ts">
import { type SelectOption } from "naive-ui";
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
  kind: "kairox_json",
  url: "",
  api_key_env: null,
  priority: 100,
  default_trust: "community",
  enabled: true,
  cache_ttl_seconds: null
});

const sources = computed(() => catalog.sources);
const failures = computed(() => catalog.sourceFailures);

const kindOptions: SelectOption[] = [
  { label: "Kairox JSON", value: "kairox_json" },
  { label: "Smithery", value: "smithery" }
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
    kind: "kairox_json",
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
}
</script>

<template>
  <div class="catalog-sources-settings">
    <NText strong tag="h3" class="header">Remote Catalog Sources</NText>

    <NEmpty
      v-if="sources.length === 0"
      :description="t('marketplace.sourcesEmpty')"
      class="empty"
    />

    <NList v-else hoverable bordered class="src-list">
      <NListItem v-for="src in sources" :key="src.id" class="src-row">
        <div class="src-meta">
          <NSpace align="center" :size="6">
            <NText strong>{{ src.display_name }}</NText>
            <NText depth="3" code class="src-id">{{ src.id }}</NText>
            <NTag size="small" :bordered="false" type="info" class="src-kind">
              {{ src.kind }}
            </NTag>
          </NSpace>
          <a
            v-if="src.url"
            :href="src.url"
            target="_blank"
            rel="noopener noreferrer"
            class="src-url"
          >
            {{ src.url }}
          </a>
          <NText
            v-if="failures[src.id]"
            type="error"
            class="src-error"
            :title="`Last error: ${failures[src.id]}`"
          >
            ⚠ {{ failures[src.id] }}
          </NText>
        </div>
        <template #suffix>
          <NSpace align="center" :size="8" class="src-actions">
            <!-- 7c review carry-over: migrated from native <input
                 type="checkbox"> to <NCheckbox> so the row follows the
                 active NaiveUI theme. The data-test hook is forwarded
                 verbatim so existing component tests
                 ([data-test="src-enable-${id}"]) keep matching. NCheckbox
                 emits update:checked with the new boolean value, which the
                 setValue('checked') / .vm.$emit pattern in tests already
                 supports. -->
            <NCheckbox
              :checked="src.enabled"
              :disabled="src.id === 'builtin'"
              :data-test="`src-enable-${src.id}`"
              size="small"
              class="src-enable"
              @update:checked="(checked) => onToggleChecked(src.id, checked)"
            >
              Enabled
            </NCheckbox>
            <NButton
              v-if="src.id !== 'builtin'"
              size="tiny"
              :data-test="`src-remove-${src.id}`"
              type="error"
              ghost
              @click="onRemove(src.id)"
            >
              Remove
            </NButton>
          </NSpace>
        </template>
      </NListItem>
    </NList>

    <NButton
      v-if="!showAddForm"
      data-test="add-source-toggle"
      size="small"
      @click="showAddForm = true"
    >
      + Add source
    </NButton>

    <div v-else class="add-form">
      <label class="field">
        <NText depth="2">id</NText>
        <NInput
          v-model:value="draft.id"
          size="small"
          :input-props="{ 'data-test': 'src-id' }"
        />
      </label>
      <label class="field">
        <NText depth="2">display name</NText>
        <NInput
          v-model:value="draft.display_name"
          size="small"
          :input-props="{ 'data-test': 'src-name' }"
        />
      </label>
      <label class="field">
        <NText depth="2">kind</NText>
        <NSelect
          v-model:value="draft.kind"
          :options="kindOptions"
          size="small"
        />
      </label>
      <label class="field">
        <NText depth="2">url</NText>
        <NInput
          v-model:value="draft.url"
          size="small"
          :input-props="{ 'data-test': 'src-url' }"
        />
      </label>
      <label class="field">
        <NText depth="2">api_key_env</NText>
        <NInput v-model:value="draft.api_key_env" size="small" />
      </label>
      <NText v-if="formError" type="error" class="error">
        {{ formError }}
      </NText>
      <NSpace class="form-actions" :size="8">
        <NButton size="small" type="primary" data-test="src-save" @click="save">
          Save
        </NButton>
        <NButton
          size="small"
          @click="
            showAddForm = false;
            formError = null;
          "
        >
          Cancel
        </NButton>
      </NSpace>
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
}
.empty {
  font-style: italic;
}
.src-list {
  border-radius: 4px;
}
.src-row {
  padding: 8px 12px;
}
.src-meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.src-id {
  font-size: 0.85em;
}
.src-kind {
  font-size: 0.7em;
  text-transform: uppercase;
}
.src-url {
  font-size: 0.85em;
}
.src-error {
  font-size: 0.85em;
}
.src-actions {
  flex-shrink: 0;
}
.src-enable {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 0.85em;
  cursor: pointer;
}
.add-form {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border: 1px dashed var(--app-border-color, #ddd);
  border-radius: 4px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.error {
  margin: 0;
}
.form-actions {
  margin-top: 4px;
}
</style>

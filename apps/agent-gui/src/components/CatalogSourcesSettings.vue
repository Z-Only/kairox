<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import { useCatalogStore } from "@/stores/catalog";
import type { AddCatalogSourceRequestPayload } from "../generated/commands";

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
    formError.value = "id and display_name are required";
    return;
  }
  if (!isValidUrl(draft.value.url)) {
    formError.value = "URL must start with http:// or https://";
    return;
  }
  await catalog.addSource({ ...draft.value });
  showAddForm.value = false;
  resetDraft();
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
    <h3>Remote Catalog Sources</h3>

    <p v-if="sources.length === 0" class="empty">No remote catalog sources configured.</p>

    <ul v-else class="src-list">
      <li v-for="src in sources" :key="src.id" class="src-row">
        <div class="src-meta">
          <strong>{{ src.display_name }}</strong>
          <code class="src-id">{{ src.id }}</code>
          <span class="src-kind">{{ src.kind }}</span>
          <a v-if="src.url" :href="src.url" target="_blank" rel="noopener noreferrer">
            {{ src.url }}
          </a>
          <span v-if="failures[src.id]" class="src-error" title="Last error">
            ⚠ {{ failures[src.id] }}
          </span>
        </div>
        <div class="src-actions">
          <label class="src-enable">
            <input
              type="checkbox"
              :checked="src.enabled"
              :disabled="src.id === 'builtin'"
              :data-test="`src-enable-${src.id}`"
              @change="onToggle(src.id, ($event.target as HTMLInputElement).checked)"
            />
            Enabled
          </label>
          <button
            v-if="src.id !== 'builtin'"
            :data-test="`src-remove-${src.id}`"
            type="button"
            @click="onRemove(src.id)"
          >
            Remove
          </button>
        </div>
      </li>
    </ul>

    <button
      v-if="!showAddForm"
      data-test="add-source-toggle"
      type="button"
      @click="showAddForm = true"
    >
      + Add source
    </button>

    <div v-else class="add-form">
      <label>
        id
        <input v-model="draft.id" data-test="src-id" />
      </label>
      <label>
        display name
        <input v-model="draft.display_name" data-test="src-name" />
      </label>
      <label>
        kind
        <select v-model="draft.kind">
          <option value="kairox_json">Kairox JSON</option>
          <option value="smithery">Smithery</option>
        </select>
      </label>
      <label>
        url
        <input v-model="draft.url" data-test="src-url" />
      </label>
      <label>
        api_key_env
        <input v-model="draft.api_key_env" />
      </label>
      <p v-if="formError" class="error">{{ formError }}</p>
      <div class="form-actions">
        <button data-test="src-save" type="button" @click="save">Save</button>
        <button
          type="button"
          @click="
            showAddForm = false;
            formError = null;
          "
        >
          Cancel
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

.empty {
  color: var(--text-muted, #888);
  font-style: italic;
}

.src-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.src-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border: 1px solid var(--border, #ddd);
  border-radius: 4px;
}

.src-meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.src-id {
  font-size: 0.85em;
  color: var(--text-muted, #888);
}

.src-kind {
  font-size: 0.8em;
  text-transform: uppercase;
  color: var(--accent, #4a8);
}

.src-error {
  color: var(--danger, #c33);
  font-size: 0.85em;
}

.src-actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.add-form {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border: 1px dashed var(--border, #ddd);
  border-radius: 4px;
}

.error {
  color: var(--danger, #c33);
  margin: 0;
}

.form-actions {
  display: flex;
  gap: 8px;
}
</style>

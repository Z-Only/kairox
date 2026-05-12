<script setup lang="ts">
import { useSkillsStore } from "@/stores/skills";
import SkillDiscoverCard from "./SkillDiscoverCard.vue";
import type { SkillCatalogQuery } from "@/generated/commands";

const { t } = useI18n();
const store = useSkillsStore();
const searchKeyword = ref("");
const installingId = ref<string | null>(null);

const sourceOptions = computed<{ label: string; value: string }[]>(() => [
  { label: "All sources", value: "" },
  ...store.catalogSources
    .filter((s) => s.enabled)
    .map((s) => ({ label: s.display_name, value: s.id }))
]);

onMounted(async () => {
  await store.loadCatalogSources();
  if (store.catalogEntries.length === 0) {
    await searchCatalog();
  }
});

async function searchCatalog(): Promise<void> {
  const query: SkillCatalogQuery = {
    keyword: searchKeyword.value.trim() || null,
    sources: null,
    limit: 50
  };
  await store.searchCatalog(query);
}

function onSearchInput(): void {
  void searchCatalog();
}

async function onInstall(entryPackage: string): Promise<void> {
  installingId.value = entryPackage;
  try {
    await store.installRemoteSkill(entryPackage, "user");
  } finally {
    installingId.value = null;
  }
}
</script>

<template>
  <div class="discover-list">
    <div class="filters">
      <input
        v-model="searchKeyword"
        class="filter-keyword"
        type="search"
        :placeholder="t('skills.searchPlaceholder')"
        data-test="skill-catalog-search"
        @input="onSearchInput"
      />
      <select class="filter-source" data-test="skill-catalog-source-filter" @change="searchCatalog">
        <option v-for="opt in sourceOptions" :key="opt.value" :value="opt.value">
          {{ opt.label }}
        </option>
      </select>
      <button
        class="btn btn-sm"
        type="button"
        data-test="skill-catalog-refresh"
        @click="store.refreshCatalog().then(() => searchCatalog())"
      >
        {{ t("common.refresh") }}
      </button>
    </div>

    <div v-if="store.catalogLoading" class="loading" role="status">
      <span class="spinner" />
      <span class="text-secondary">{{ t("common.loading") }}</span>
    </div>
    <span v-else-if="store.error" class="text-error error" role="alert">
      {{ store.error }}
    </span>
    <p v-else-if="store.catalogEntries.length === 0" class="empty-state">
      {{ t("skills.catalogEmpty") }}
    </p>
    <div v-else class="grid">
      <SkillDiscoverCard
        v-for="entry in store.catalogEntries"
        :key="entry.catalog_id"
        :entry="entry"
        :installing="installingId === entry.package"
        @install="onInstall(entry.package)"
      />
    </div>
  </div>
</template>

<style scoped>
.discover-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.filters {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
  align-items: center;
}

.filter-keyword {
  flex: 1;
  max-width: 280px;
  min-height: 32px;
  padding: 4px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  font-size: 13px;
  background: var(--app-card-color);
  color: var(--app-text-color);
}

.filter-source {
  width: 180px;
  min-height: 32px;
  padding: 4px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 4px;
  font-size: 13px;
  background: var(--app-card-color);
  color: var(--app-text-color);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 12px;
}

.loading {
  display: flex;
  align-items: center;
  gap: 8px;
}

.text-secondary {
  color: var(--app-text-color-2);
}

.text-error {
  color: var(--app-error-color);
}

.empty-state {
  text-align: center;
  font-style: italic;
  color: var(--app-text-color-3);
  padding: 24px 0;
}

.spinner {
  width: 16px;
  height: 16px;
  border: 2px solid var(--app-border-color);
  border-top-color: var(--app-primary-color);
  border-radius: 50%;
  display: inline-block;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useCatalogStore } from "@/stores/catalog";
import CatalogCard from "./CatalogCard.vue";
import CatalogDetail from "./CatalogDetail.vue";
import type { ServerEntryResponse } from "../../generated/commands";

interface TrustOption {
  label: string;
  value: string | null;
}

const { t } = useI18n();
const catalog = useCatalogStore();
const selected = ref<ServerEntryResponse | null>(null);
const searchInput = ref("");

const trustOptions = computed<TrustOption[]>(() => [
  { label: t("marketplace.trustAll"), value: null },
  { label: t("marketplace.trustVerified"), value: "verified" },
  { label: t("marketplace.trustCommunity"), value: "community" }
]);

async function handleRefresh() {
  catalog.filters.keyword = searchInput.value;
  if (searchInput.value.trim()) {
    await catalog.fetchCatalog({ keyword: searchInput.value.trim() });
  } else {
    await catalog.refreshCatalogSource(null);
  }
}

onMounted(async () => {
  if (catalog.entries.length === 0) {
    await catalog.fetchCatalog();
  }
});
</script>

<template>
  <div class="catalog-list">
    <div class="filters">
      <input
        v-model="searchInput"
        :placeholder="t('marketplace.searchServers')"
        data-test="catalog-search"
        class="filter-keyword"
        autocapitalize="off"
        autocomplete="off"
        spellcheck="false"
        @keyup.enter="handleRefresh"
      />
      <select
        :value="catalog.filters.trustMin ?? ''"
        data-test="catalog-trust"
        class="filter-trust"
        @change="catalog.filters.trustMin = ($event.target as HTMLSelectElement).value || null"
      >
        <option v-for="opt in trustOptions" :key="String(opt.value)" :value="opt.value ?? ''">
          {{ opt.label }}
        </option>
      </select>
      <button class="btn btn-sm" data-test="catalog-refresh" @click="handleRefresh">
        {{ t("common.refresh") }}
      </button>
    </div>

    <KxStateBlock v-if="catalog.loading" tone="loading" compact data-test="catalog-loading-state">
      <span class="spinner" />
      <span class="text-secondary">{{ t("common.loading") }}</span>
    </KxStateBlock>
    <KxStateBlock v-else-if="catalog.error" tone="error" compact data-test="catalog-error-state">
      {{ catalog.error }}
    </KxStateBlock>
    <KxStateBlock
      v-else-if="catalog.visibleEntries.length === 0"
      tone="empty"
      data-test="catalog-empty-state"
    >
      {{ t("marketplace.catalogEmpty") }}
    </KxStateBlock>
    <div v-else class="scrollable-area">
      <div class="grid">
        <CatalogCard
          v-for="entry in catalog.visibleEntries"
          :key="entry.id"
          :entry="entry"
          @click="selected = entry"
        />
      </div>
    </div>
    <CatalogDetail v-if="selected" :entry="selected" @close="selected = null" />
  </div>
</template>

<style scoped>
.catalog-list {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow: hidden;
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
}

.filter-trust {
  width: 180px;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 12px;
  padding-right: 4px;
}

.loading {
  display: flex;
  align-items: center;
  gap: 8px;
}

.scrollable-area {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.text-secondary {
  color: var(--app-text-color-2);
}

.text-error {
  color: var(--app-error-color);
}
</style>

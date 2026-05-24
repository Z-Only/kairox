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

type CatalogSort = "name" | "trust" | "source";

const TRUST_ORDER: Record<string, number> = {
  unverified: 0,
  community: 1,
  verified: 2
};

const { t } = useI18n();
const catalog = useCatalogStore();
const selected = ref<ServerEntryResponse | null>(null);
const catalogSort = ref<CatalogSort>("name");
const searchInput = computed({
  get: () => catalog.filters.keyword,
  set: (value: string) => {
    catalog.filters.keyword = value;
  }
});

const trustOptions = computed<TrustOption[]>(() => [
  { label: t("marketplace.trustAll"), value: null },
  { label: t("marketplace.trustVerified"), value: "verified" },
  { label: t("marketplace.trustCommunity"), value: "community" }
]);

const sortedVisibleEntries = computed(() => {
  const byName = (a: ServerEntryResponse, b: ServerEntryResponse) =>
    a.display_name.localeCompare(b.display_name);
  return [...catalog.visibleEntries].sort((a, b) => {
    if (catalogSort.value === "trust") {
      const trustDelta = (TRUST_ORDER[b.trust] ?? 0) - (TRUST_ORDER[a.trust] ?? 0);
      if (trustDelta !== 0) return trustDelta;
      return byName(a, b);
    }
    if (catalogSort.value === "source") {
      const sourceDelta = a.source.localeCompare(b.source);
      if (sourceDelta !== 0) return sourceDelta;
      return byName(a, b);
    }
    return byName(a, b);
  });
});

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
    <SettingsFilterBar class="filters" :aria-label="t('marketplace.title')">
      <div class="settings-filter-bar__row">
        <KxInput
          v-model="searchInput"
          type="search"
          :placeholder="t('marketplace.searchServers')"
          data-test="catalog-search"
          class="filter-keyword"
          autocapitalize="off"
          autocomplete="off"
          spellcheck="false"
          @keyup.enter="handleRefresh"
        />
        <KxSelect
          :model-value="catalog.filters.trustMin ?? ''"
          data-test="catalog-trust"
          class="filter-trust"
          size="compact"
          @update:model-value="catalog.filters.trustMin = $event || null"
        >
          <option v-for="opt in trustOptions" :key="String(opt.value)" :value="opt.value ?? ''">
            {{ opt.label }}
          </option>
        </KxSelect>
        <KxSelect
          v-model="catalogSort"
          data-test="catalog-sort"
          aria-label="Catalog sort"
          class="filter-sort"
          size="compact"
        >
          <option value="name">Name</option>
          <option value="trust">Trust</option>
          <option value="source">Source</option>
        </KxSelect>
        <KxToolbarAction data-test="catalog-refresh" @click="handleRefresh">
          {{ t("common.refresh") }}
        </KxToolbarAction>
      </div>
    </SettingsFilterBar>

    <SettingsState v-if="catalog.loading" tone="loading" data-test="catalog-loading-state">
      {{ t("common.loading") }}
    </SettingsState>
    <SettingsState v-else-if="catalog.error" tone="error" data-test="catalog-error-state">
      {{ catalog.error }}
    </SettingsState>
    <SettingsState
      v-else-if="catalog.visibleEntries.length === 0"
      tone="empty"
      data-test="catalog-empty-state"
    >
      {{ t("marketplace.catalogEmpty") }}
    </SettingsState>
    <div v-else class="scrollable-area">
      <div class="grid">
        <CatalogCard
          v-for="entry in sortedVisibleEntries"
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

.filter-keyword {
  flex: 1;
  max-width: 280px;
}

.filter-trust {
  width: 180px;
}

.filter-sort {
  width: 120px;
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

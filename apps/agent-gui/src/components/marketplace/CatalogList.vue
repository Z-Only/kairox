<script setup lang="ts">
import { type SelectOption } from "naive-ui";
import { useI18n } from "vue-i18n";
import { useCatalogStore } from "@/stores/catalog";
import CatalogCard from "./CatalogCard.vue";
import CatalogDetail from "./CatalogDetail.vue";
import type { ServerEntryResponse } from "../../generated/commands";

const { t } = useI18n();
const catalog = useCatalogStore();
const selected = ref<ServerEntryResponse | null>(null);

const trustOptions = computed<SelectOption[]>(() => [
  { label: "All trust levels", value: null as unknown as string },
  { label: "Verified+", value: "verified" },
  { label: "Community+", value: "community" }
]);

onMounted(async () => {
  if (catalog.entries.length === 0) {
    await catalog.fetchCatalog();
  }
});
</script>

<template>
  <div class="catalog-list">
    <div class="filters">
      <!-- Filter inputs expose data-test hooks for the existing test suite.
           NInput passes data-test through to its underlying <input> via
           input-props; NSelect attaches data-test to its root element so
           component tests can locate it with findComponent({name:'NSelect'})
           or selectors targeting [data-test="catalog-trust"]. -->
      <NInput
        v-model:value="catalog.filters.keyword"
        placeholder="Search servers…"
        clearable
        size="small"
        :input-props="{ 'data-test': 'catalog-search' }"
        class="filter-keyword"
      />
      <NSelect
        v-model:value="catalog.filters.trustMin"
        :options="trustOptions"
        size="small"
        class="filter-trust"
        :consistent-menu-width="false"
        placeholder="All trust levels"
        data-test="catalog-trust"
      />
      <NButton size="small" data-test="catalog-refresh" @click="catalog.refreshCatalogSource(null)">
        Refresh
      </NButton>
    </div>

    <div v-if="catalog.loading" class="loading">
      <NSpin size="small" />
      <NText depth="2">Loading…</NText>
    </div>
    <NText v-else-if="catalog.error" type="error" class="error">
      {{ catalog.error }}
    </NText>
    <NEmpty
      v-else-if="catalog.visibleEntries.length === 0"
      :description="t('marketplace.catalogEmpty')"
    />
    <NScrollbar v-else style="max-height: calc(100vh - 320px)">
      <div class="grid">
        <CatalogCard
          v-for="entry in catalog.visibleEntries"
          :key="entry.id"
          :entry="entry"
          @click="selected = entry"
        />
      </div>
    </NScrollbar>
    <CatalogDetail v-if="selected" :entry="selected" @close="selected = null" />
  </div>
</template>

<style scoped>
.catalog-list {
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
</style>

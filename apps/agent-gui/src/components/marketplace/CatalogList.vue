<script setup lang="ts">
import { ref, onMounted } from "vue";
import {
  catalogState,
  visibleEntries,
  fetchCatalog,
  refreshCatalogSource
} from "../../stores/catalog";
import CatalogCard from "./CatalogCard.vue";
import CatalogDetail from "./CatalogDetail.vue";
import type { ServerEntryResponse } from "../../generated/commands";

const selected = ref<ServerEntryResponse | null>(null);

onMounted(async () => {
  if (catalogState.entries.length === 0) {
    await fetchCatalog();
  }
});
</script>

<template>
  <div class="catalog-list">
    <div class="filters">
      <input
        v-model="catalogState.filters.keyword"
        placeholder="Search servers…"
        data-test="catalog-search"
      />
      <select v-model="catalogState.filters.trustMin" data-test="catalog-trust">
        <option :value="null">All trust levels</option>
        <option value="verified">Verified+</option>
        <option value="community">Community+</option>
      </select>
      <button data-test="catalog-refresh" @click="refreshCatalogSource(null)">Refresh</button>
    </div>
    <p v-if="catalogState.loading">Loading…</p>
    <p v-else-if="catalogState.error" class="error">
      {{ catalogState.error }}
    </p>
    <div v-else class="grid">
      <CatalogCard
        v-for="entry in visibleEntries"
        :key="entry.id"
        :entry="entry"
        @click="selected = entry"
      />
    </div>
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
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 12px;
}
.error {
  color: var(--error, #c33);
}
</style>

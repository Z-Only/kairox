<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useCatalogStore } from "@/stores/catalog";
import CatalogCard from "./CatalogCard.vue";
import CatalogDetail from "./CatalogDetail.vue";
import type { ServerEntryResponse } from "../../generated/commands";

const catalog = useCatalogStore();
const selected = ref<ServerEntryResponse | null>(null);

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
        v-model="catalog.filters.keyword"
        placeholder="Search servers…"
        data-test="catalog-search"
      />
      <select v-model="catalog.filters.trustMin" data-test="catalog-trust">
        <option :value="null">All trust levels</option>
        <option value="verified">Verified+</option>
        <option value="community">Community+</option>
      </select>
      <button
        data-test="catalog-refresh"
        @click="catalog.refreshCatalogSource(null)"
      >
        Refresh
      </button>
    </div>
    <p v-if="catalog.loading">Loading…</p>
    <p v-else-if="catalog.error" class="error">
      {{ catalog.error }}
    </p>
    <div v-else class="grid">
      <CatalogCard
        v-for="entry in catalog.visibleEntries"
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

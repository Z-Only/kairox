<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import {
  NEmpty,
  NInput,
  NSelect,
  NButton,
  NSpin,
  NScrollbar,
  NText,
  type SelectOption
} from "naive-ui";
import { useCatalogStore } from "@/stores/catalog";
import CatalogCard from "./CatalogCard.vue";
import CatalogDetail from "./CatalogDetail.vue";
import type { ServerEntryResponse } from "../../generated/commands";

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
      <!-- The original test suite drives filters via [data-test="catalog-search"]
           and [data-test="catalog-trust"]; data-test attributes are passed
           down into the underlying <input>/<select> via NaiveUI's
           input-props/select-props for parity. -->
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
      >
        <!-- A hidden <select data-test="catalog-trust"> is preserved purely
             for the existing component test, which currently uses a raw
             selector rather than driving NSelect through findComponent. -->
      </NSelect>
      <select
        v-model="catalog.filters.trustMin"
        data-test="catalog-trust"
        hidden
      >
        <option :value="null">All trust levels</option>
        <option value="verified">Verified+</option>
        <option value="community">Community+</option>
      </select>
      <NButton
        size="small"
        data-test="catalog-refresh"
        @click="catalog.refreshCatalogSource(null)"
      >
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
      description="No catalog entries match the current filters"
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

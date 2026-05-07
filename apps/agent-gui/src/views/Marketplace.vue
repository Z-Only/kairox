<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { catalogState, fetchSources, isSourceSelected, toggleSource } from "../stores/catalog";
import CatalogList from "../components/marketplace/CatalogList.vue";
import InstalledList from "../components/marketplace/InstalledList.vue";
import CatalogSourcesSettings from "../components/CatalogSourcesSettings.vue";

const installedCount = computed(() => catalogState.installed.length);
const settingsOpen = ref(false);

function setTab(tab: "browse" | "installed") {
  catalogState.tab = tab;
}

const sourceChips = computed(() => [
  { id: "builtin", display_name: "Built-in" },
  ...catalogState.sources.map((s) => ({
    id: s.id,
    display_name: s.display_name
  }))
]);

onMounted(() => {
  void fetchSources();
});
</script>

<template>
  <section class="marketplace">
    <header class="marketplace__header">
      <h1>Marketplace</h1>
      <nav class="tabs">
        <button
          data-test="tab-browse"
          :class="{ active: catalogState.tab === 'browse' }"
          @click="setTab('browse')"
        >
          Browse
        </button>
        <button
          data-test="tab-installed"
          :class="{ active: catalogState.tab === 'installed' }"
          @click="setTab('installed')"
        >
          Installed ({{ installedCount }})
        </button>
      </nav>
    </header>
    <div v-if="catalogState.tab === 'browse'" class="source-filter">
      <button
        v-for="chip in sourceChips"
        :key="chip.id"
        :data-test="`source-chip-${chip.id}`"
        :class="{ chip: true, active: isSourceSelected(chip.id) }"
        type="button"
        @click="toggleSource(chip.id)"
      >
        {{ chip.display_name }}
        <span
          v-if="catalogState.sourceFailures[chip.id]"
          :data-test="`src-warn-${chip.id}`"
          :title="catalogState.sourceFailures[chip.id]"
          class="warn"
          >⚠</span
        >
      </button>
      <button
        type="button"
        class="settings-icon"
        data-test="catalog-source-settings"
        aria-label="Catalog source settings"
        @click="settingsOpen = !settingsOpen"
      >
        ⚙
      </button>
    </div>
    <div
      v-if="settingsOpen && catalogState.tab === 'browse'"
      class="settings-drawer"
      data-test="catalog-source-settings-drawer"
    >
      <CatalogSourcesSettings />
    </div>
    <CatalogList v-if="catalogState.tab === 'browse'" />
    <InstalledList v-else />
  </section>
</template>

<style scoped>
.marketplace {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 16px;
  overflow-y: auto;
}
.marketplace__header {
  display: flex;
  align-items: baseline;
  gap: 24px;
}
.tabs {
  display: flex;
  gap: 8px;
}
.tabs button {
  padding: 6px 12px;
  border: 1px solid var(--border, #ccc);
  background: transparent;
  cursor: pointer;
}
.tabs button.active {
  background: var(--accent, #345);
  color: #fff;
}
.source-filter {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}
.chip {
  padding: 4px 10px;
  border: 1px solid var(--border, #ccc);
  border-radius: 12px;
  background: transparent;
  cursor: pointer;
  font-size: 0.85em;
}
.chip.active {
  background: var(--accent, #345);
  color: #fff;
  border-color: var(--accent, #345);
}
.warn {
  margin-left: 4px;
  color: #c00;
}
.settings-icon {
  margin-left: auto;
  padding: 4px 8px;
  border: 1px solid var(--border, #ccc);
  border-radius: 4px;
  background: transparent;
  cursor: pointer;
  font-size: 1em;
}
.settings-drawer {
  padding: 12px;
  border: 1px solid var(--border, #ddd);
  border-radius: 6px;
  background: var(--surface, #fafafa);
}
</style>

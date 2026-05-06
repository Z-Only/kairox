<script setup lang="ts">
import { computed } from "vue";
import { catalogState } from "../stores/catalog";
import CatalogList from "../components/marketplace/CatalogList.vue";
import InstalledList from "../components/marketplace/InstalledList.vue";

const installedCount = computed(() => catalogState.installed.length);

function setTab(tab: "browse" | "installed") {
  catalogState.tab = tab;
}
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
</style>

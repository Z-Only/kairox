<script setup lang="ts">
import { useCatalogStore } from "@/stores/catalog";
import CatalogList from "@/components/marketplace/CatalogList.vue";
import InstalledList from "@/components/marketplace/InstalledList.vue";
import InstallProgress from "@/components/marketplace/InstallProgress.vue";
import CatalogSourcesSettings from "@/components/CatalogSourcesSettings.vue";

const catalog = useCatalogStore();
const { t } = useI18n();
const installedCount = computed(() => catalog.installed.length);
const settingsOpen = ref(false);

const sourceChips = computed(() => {
  const remoteSources = catalog.sources
    .filter((s) => s.id !== "builtin")
    .map((s) => ({
      id: s.id,
      display_name: s.display_name
    }));
  return [{ id: "builtin", display_name: t("marketplace.builtinSource") }, ...remoteSources];
});

onMounted(async () => {
  await catalog.fetchSources();
  const hasEnabledRemote = catalog.sources.some((s) => s.id !== "builtin" && s.enabled);
  if (hasEnabledRemote) {
    await catalog.refreshCatalogSource();
  } else {
    await catalog.fetchCatalog();
  }
});
</script>

<template>
  <div class="marketplace-pane">
    <header class="marketplace-pane__header">
      <h1 class="marketplace-pane__title">
        {{ t("marketplace.title") }}
      </h1>
    </header>

    <div class="marketplace-tabs">
      <div class="tabs">
        <button
          data-test="tab-browse"
          :class="['tab-btn', { active: catalog.tab === 'browse' }]"
          @click="catalog.tab = 'browse'"
        >
          {{ t("marketplace.tabBrowse") }}
        </button>
        <button
          data-test="tab-installed"
          :class="['tab-btn', { active: catalog.tab === 'installed' }]"
          @click="catalog.tab = 'installed'"
        >
          {{ t("marketplace.tabInstalled", { count: installedCount }) }}
        </button>
      </div>

      <div v-show="catalog.tab === 'browse'">
        <div class="source-filter">
          <button
            v-for="chip in sourceChips"
            :key="chip.id"
            :data-test="`source-chip-${chip.id}`"
            :class="['btn', 'chip', { active: catalog.isSourceEnabled(chip.id) }]"
            @click="catalog.toggleSource(chip.id)"
          >
            {{ chip.display_name }}
            <span
              v-if="catalog.sourceFailures[chip.id]"
              :data-test="`src-warn-${chip.id}`"
              :title="catalog.sourceFailures[chip.id]"
              class="tag tag-error warn"
            >
              ⚠
            </span>
          </button>
          <button
            class="btn settings-icon"
            data-test="catalog-source-settings"
            :aria-label="t('marketplace.sourceSettingsAria')"
            @click="settingsOpen = !settingsOpen"
          >
            <span aria-hidden="true">⚙</span>
          </button>
        </div>

        <div
          v-if="settingsOpen"
          class="card settings-drawer"
          data-test="catalog-source-settings-drawer"
        >
          <CatalogSourcesSettings />
        </div>

        <CatalogList />
      </div>

      <div v-show="catalog.tab === 'installed'">
        <InstalledList />
      </div>
    </div>

    <InstallProgress
      v-if="catalog.currentInstallEntryId"
      :catalog-id="catalog.currentInstallEntryId"
      @close="catalog.dismissInstallProgress()"
    />
  </div>
</template>

<style scoped>
.marketplace-pane {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.marketplace-pane__title {
  margin: 0;
  font-size: 20px;
}
.tabs {
  display: flex;
  gap: 8px;
  border-bottom: 1px solid var(--app-border-color, #e0e0e0);
  margin-bottom: 12px;
}
.tab-btn {
  padding: 6px 14px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: 14px;
  color: var(--app-text-color, inherit);
  border-bottom: 2px solid transparent;
  transition:
    border-color 0.2s,
    color 0.2s;
}
.tab-btn:hover {
  color: var(--app-primary-color, #18a058);
}
.tab-btn.active {
  color: var(--app-primary-color, #18a058);
  border-bottom-color: var(--app-primary-color, #18a058);
}
.btn {
  padding: 4px 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 14px;
  background: var(--app-card-color);
  cursor: pointer;
  color: var(--app-text-color);
}
.btn.active {
  background: var(--app-primary-color, #18a058);
  color: #fff;
  border-color: var(--app-primary-color, #18a058);
}
.tag-error {
  color: var(--app-error-color, #d03050);
  font-size: 0.85em;
}
.card {
  border: 1px solid var(--app-border-color, #e0e0e0);
  border-radius: 4px;
  padding: 12px;
}
.source-filter {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  margin-bottom: 12px;
}
.chip {
  font-size: 0.85em;
}
.warn {
  margin-left: 4px;
}
.settings-icon {
  margin-left: auto;
}
.settings-drawer {
  margin-bottom: 12px;
}
</style>

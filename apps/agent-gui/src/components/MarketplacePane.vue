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

onMounted(() => {
  void catalog.fetchSources();
  void catalog.fetchCatalog();
});
</script>

<template>
  <div class="marketplace-pane">
    <header class="marketplace-pane__header">
      <NText tag="h1" :depth="1" class="marketplace-pane__title">
        {{ t("marketplace.title") }}
      </NText>
    </header>

    <NTabs v-model:value="catalog.tab" type="line" animated size="medium" class="marketplace-tabs">
      <NTabPane name="browse">
        <template #tab>
          <span data-test="tab-browse">{{ t("marketplace.tabBrowse") }}</span>
        </template>

        <div class="source-filter">
          <NButton
            v-for="chip in sourceChips"
            :key="chip.id"
            :data-test="`source-chip-${chip.id}`"
            :type="catalog.isSourceSelected(chip.id) ? 'primary' : 'default'"
            size="small"
            round
            :class="['chip', { active: catalog.isSourceSelected(chip.id) }]"
            @click="catalog.toggleSource(chip.id)"
          >
            {{ chip.display_name }}
            <NTag
              v-if="catalog.sourceFailures[chip.id]"
              size="small"
              type="error"
              :bordered="false"
              :data-test="`src-warn-${chip.id}`"
              :title="catalog.sourceFailures[chip.id]"
              class="warn"
            >
              ⚠
            </NTag>
          </NButton>
          <NButton
            quaternary
            circle
            size="small"
            class="settings-icon"
            data-test="catalog-source-settings"
            :aria-label="t('marketplace.sourceSettingsAria')"
            @click="settingsOpen = !settingsOpen"
          >
            <NIcon>
              <span aria-hidden="true">⚙</span>
            </NIcon>
          </NButton>
        </div>

        <NCard
          v-if="settingsOpen"
          size="small"
          :bordered="true"
          class="settings-drawer"
          data-test="catalog-source-settings-drawer"
        >
          <CatalogSourcesSettings />
        </NCard>

        <CatalogList />
      </NTabPane>

      <NTabPane name="installed">
        <template #tab>
          <span data-test="tab-installed">
            {{ t("marketplace.tabInstalled", { count: installedCount }) }}
          </span>
        </template>
        <InstalledList />
      </NTabPane>
    </NTabs>

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
.marketplace-pane__header :deep(h1.marketplace-pane__title) {
  margin: 0;
  font-size: 20px;
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

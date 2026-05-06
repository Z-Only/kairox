<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { NCard, NTabs, NTabPane, NButton, NTag, NIcon } from "naive-ui";
import { useCatalogStore } from "@/stores/catalog";
import CatalogList from "@/components/marketplace/CatalogList.vue";
import InstalledList from "@/components/marketplace/InstalledList.vue";
import CatalogSourcesSettings from "@/components/CatalogSourcesSettings.vue";

const catalog = useCatalogStore();
const { t } = useI18n();
const installedCount = computed(() => catalog.installed.length);
const settingsOpen = ref(false);

// NTabs drives `catalog.tab` directly via v-model so behaviour is identical
// to the previous custom buttons, but each NTabPane keeps the
// data-test="tab-browse" / "tab-installed" hook attached to its tab header
// for the existing Marketplace.test.ts assertions.
const sourceChips = computed(() => [
  { id: "builtin", display_name: t("marketplace.builtinSource") },
  ...catalog.sources.map((s) => ({
    id: s.id,
    display_name: s.display_name
  }))
]);

onMounted(() => {
  void catalog.fetchSources();
});
</script>

<template>
  <NCard
    class="marketplace"
    :bordered="false"
    content-style="padding: 16px; display: flex; flex-direction: column; gap: 16px;"
  >
    <header class="marketplace__header">
      <h1>{{ t("marketplace.title") }}</h1>
    </header>

    <NTabs
      v-model:value="catalog.tab"
      type="line"
      animated
      size="medium"
      class="marketplace-tabs"
    >
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
            class="chip"
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
  </NCard>
</template>

<style scoped>
.marketplace {
  height: 100%;
  overflow-y: auto;
}
.marketplace__header h1 {
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

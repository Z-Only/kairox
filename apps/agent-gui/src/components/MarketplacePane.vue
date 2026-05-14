<script setup lang="ts">
import { useCatalogStore } from "@/stores/catalog";
import CatalogList from "@/components/marketplace/CatalogList.vue";
import InstallProgress from "@/components/marketplace/InstallProgress.vue";
import CatalogSourcesSettings from "@/components/CatalogSourcesSettings.vue";
import ModalDialog from "@/components/ui/ModalDialog.vue";

const catalog = useCatalogStore();
const { t } = useI18n();
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

const hasCachedEntries = computed(() => catalog.entries.length > 0);

onMounted(async () => {
  if (catalog.tab === "installed") {
    catalog.tab = "browse";
  }

  // If we have cached entries, show them immediately and refresh in background.
  if (hasCachedEntries.value) {
    catalog.fetchSources();
    const hasEnabledRemote = catalog.sources.some((s) => s.id !== "builtin" && s.enabled);
    if (hasEnabledRemote) {
      catalog.refreshCatalogSource();
    }
    return;
  }

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
    <div>
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

      <ModalDialog
        :open="settingsOpen"
        :title="t('marketplace.sourceSettingsAria')"
        data-test="catalog-source-settings-drawer"
        @close="settingsOpen = false"
      >
        <CatalogSourcesSettings />
      </ModalDialog>

      <CatalogList />
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

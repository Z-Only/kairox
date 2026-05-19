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

onMounted(async () => {
  if (catalog.tab === "installed") {
    catalog.tab = "browse";
  }

  // Always wait for the source list so we can decide whether to
  // trigger a remote refresh. The backend caches catalog entries in
  // memory per session, so fetchCatalog is fast after the first load.
  await catalog.fetchSources();
  const hasEnabledRemote = catalog.sources.some((s) => s.id !== "builtin" && s.enabled);

  // Cold cache: block until we have data. Backend in-memory cache is
  // populated on this first call so subsequent visits are instant.
  if (catalog.entries.length === 0) {
    if (hasEnabledRemote) {
      await catalog.refreshCatalogSource();
    } else {
      await catalog.fetchCatalog();
    }
    return;
  }

  // Warm cache: show cached entries immediately, refresh remote in
  // background so fresh data replaces stale cache entries.
  if (hasEnabledRemote) {
    catalog.refreshCatalogSource();
  }
});
</script>

<template>
  <div class="marketplace-pane">
    <div>
      <KxChipGroup :aria-label="t('marketplace.title')" data-test="marketplace-source-filter">
        <KxChipButton
          v-for="chip in sourceChips"
          :key="chip.id"
          :data-test="`source-chip-${chip.id}`"
          size="compact"
          :selected="catalog.isSourceEnabled(chip.id)"
          @click="catalog.toggleSource(chip.id)"
        >
          {{ chip.display_name }}
          <span
            v-if="catalog.sourceFailures[chip.id]"
            :data-test="`src-warn-${chip.id}`"
            :title="catalog.sourceFailures[chip.id]"
            class="tag tag-error warn"
          >
            !
          </span>
        </KxChipButton>
        <template #actions>
          <KxIconButton
            :label="t('marketplace.sourceSettingsAria')"
            data-test="catalog-source-settings"
            @click="settingsOpen = !settingsOpen"
          >
            <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
              <path
                d="M8.95 2h2.1l.32 2.15c.5.17.97.42 1.4.73l2.01-.81 1.05 1.82-1.69 1.35c.05.25.08.51.08.76s-.03.51-.08.76l1.69 1.35-1.05 1.82-2.01-.81c-.43.31-.9.56-1.4.73L11.05 14h-2.1l-.32-2.15c-.5-.17-.97-.42-1.4-.73l-2.01.81-1.05-1.82 1.69-1.35A3.87 3.87 0 0 1 5.78 8c0-.25.03-.51.08-.76L4.17 5.89l1.05-1.82 2.01.81c.43-.31.9-.56 1.4-.73L8.95 2Zm1.05 4.2a1.8 1.8 0 1 0 0 3.6 1.8 1.8 0 0 0 0-3.6Z"
              />
            </svg>
          </KxIconButton>
        </template>
      </KxChipGroup>

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
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow: hidden;
}
.marketplace-pane > div:first-of-type {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
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
.kx-chip-group {
  margin-bottom: 12px;
}
.warn {
  margin-left: 4px;
}
.kx-icon-button svg {
  width: 16px;
  height: 16px;
  fill: currentColor;
}
.settings-drawer {
  margin-bottom: 12px;
}
</style>

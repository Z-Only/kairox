<script setup lang="ts">
import { useSkillsStore } from "@/stores/skills";
import type { SkillCatalogEntry, SkillInstallTarget } from "@/generated/commands";
import SkillDiscoverCard from "./SkillDiscoverCard.vue";
import SkillCatalogDetail from "./SkillCatalogDetail.vue";

const { t } = useI18n();
const store = useSkillsStore();
const props = defineProps<{ installTarget: SkillInstallTarget }>();
const installingId = ref<string | null>(null);
const selectedEntry = ref<SkillCatalogEntry | null>(null);

onMounted(async () => {
  await store.loadCatalogSources();
  if (store.catalogEntries.length === 0) {
    await store.searchCatalog({ keyword: null, sources: null, limit: 50 });
  }
});

async function onInstall(entry: SkillCatalogEntry): Promise<void> {
  installingId.value = entry.package;
  try {
    await store.installRemoteSkill(entry.package, props.installTarget, entry.package_url);
  } finally {
    installingId.value = null;
  }
}
</script>

<template>
  <div class="discover-list">
    <div v-if="store.catalogLoading" class="loading" role="status">
      <span class="spinner" />
      <span class="text-secondary">{{ t("common.loading") }}</span>
    </div>
    <span v-else-if="store.error" class="text-error error" role="alert">
      {{ store.error }}
    </span>
    <p v-else-if="store.catalogEntries.length === 0" class="empty-state">
      {{ t("skills.catalogEmpty") }}
    </p>
    <div v-else class="grid">
      <SkillDiscoverCard
        v-for="entry in store.catalogEntries"
        :key="entry.catalog_id"
        :entry="entry"
        :installing="installingId === entry.package"
        @install="onInstall(entry)"
        @select="selectedEntry = entry"
      />
    </div>
    <SkillCatalogDetail
      v-if="selectedEntry"
      :entry="selectedEntry"
      :install-target="props.installTarget"
      :installing="installingId === selectedEntry.package"
      @close="selectedEntry = null"
      @install="onInstall"
    />
  </div>
</template>

<style scoped>
.discover-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 12px;
}

.loading {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 16px 0;
}

.spinner {
  width: 16px;
  height: 16px;
  border: 2px solid var(--app-border-color);
  border-top-color: var(--app-primary-color);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.empty-state {
  padding: 24px 0;
  text-align: center;
  color: var(--app-text-color-2);
}
</style>

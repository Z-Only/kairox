<script setup lang="ts">
import { useSkillsStore } from "@/stores/skills";
import type {
  SkillCatalogEntry,
  SkillCatalogQuery,
  SkillInstallTarget
} from "@/generated/commands";
import SkillDiscoverCard from "./SkillDiscoverCard.vue";
import SkillCatalogDetail from "./SkillCatalogDetail.vue";
import SkillSourcesSettings from "./SkillSourcesSettings.vue";
import ModalDialog from "@/components/ui/ModalDialog.vue";

const { t } = useI18n();
const store = useSkillsStore();
const props = defineProps<{ installTarget: SkillInstallTarget }>();
const CATALOG_LIMIT = 100;
const installingId = ref<string | null>(null);
const installedPackages = ref<Set<string>>(new Set());
const installSuccessMessage = ref<string | null>(null);
const selectedEntry = ref<SkillCatalogEntry | null>(null);
const searchKeyword = ref("");
const selectedSourceId = ref<string | null>(null);
const sourceSettingsOpen = ref(false);

const sourceFilters = computed(() => [
  { id: "builtin", display_name: t("skills.builtinSource") },
  ...store.catalogSources.map((source) => ({
    id: source.id,
    display_name: source.display_name
  }))
]);

const selectedSourceIds = computed<string[] | null>(() =>
  selectedSourceId.value ? [selectedSourceId.value] : null
);

function buildCatalogQuery(): SkillCatalogQuery {
  return {
    keyword: searchKeyword.value.trim() || null,
    sources: selectedSourceIds.value,
    limit: CATALOG_LIMIT
  };
}

async function searchCatalog(options: { force?: boolean } = {}): Promise<void> {
  installSuccessMessage.value = null;
  await store.searchCatalog(buildCatalogQuery(), options);
}

onMounted(async () => {
  await store.loadCatalogSources();
  await searchCatalog();
});

async function onInstall(entry: SkillCatalogEntry): Promise<void> {
  if (installedPackages.value.has(entry.package)) return;
  installingId.value = entry.package;
  installSuccessMessage.value = null;
  try {
    const installedSkill = await store.installRemoteSkill(
      entry.package,
      props.installTarget,
      entry.package_url
    );
    if (installedSkill) {
      installedPackages.value = new Set([...installedPackages.value, entry.package]);
      installSuccessMessage.value = t("skills.installSuccess", { name: entry.name });
    }
  } finally {
    installingId.value = null;
  }
}

async function refreshCatalog(): Promise<void> {
  installSuccessMessage.value = null;
  await store.refreshCatalog();
  if (!store.error) {
    await searchCatalog({ force: true });
  }
}

async function selectSource(sourceId: string | null): Promise<void> {
  selectedSourceId.value = selectedSourceId.value === sourceId ? null : sourceId;
  await searchCatalog();
}
</script>

<template>
  <div class="discover-list">
    <div class="discover-toolbar">
      <div class="source-filter" :aria-label="t('skills.sourceFilter')">
        <button
          v-for="source in sourceFilters"
          :key="source.id ?? 'all'"
          type="button"
          :class="['btn', 'chip', { active: selectedSourceId === source.id }]"
          :data-test="`skill-source-filter-${source.id ?? 'all'}`"
          @click="selectSource(source.id)"
        >
          {{ source.display_name }}
        </button>
        <button
          class="btn settings-icon"
          data-test="skill-source-settings-btn"
          :aria-label="t('marketplace.sourceSettingsAria')"
          @click="sourceSettingsOpen = true"
        >
          <span aria-hidden="true">⚙</span>
        </button>
      </div>

      <form class="discover-search-row" role="search" @submit.prevent="searchCatalog()">
        <input
          v-model="searchKeyword"
          class="discover-search-input"
          type="search"
          :placeholder="t('skills.searchPlaceholder')"
          data-test="skill-catalog-search"
        />
        <button
          class="btn btn-primary btn-sm"
          type="button"
          data-test="skill-catalog-search-btn"
          @click="searchCatalog()"
        >
          {{ t("common.search") }}
        </button>
        <button
          class="btn btn-sm"
          type="button"
          data-test="skill-catalog-refresh"
          :disabled="store.catalogLoading"
          @click="refreshCatalog"
        >
          {{ store.catalogLoading ? t("skills.refreshing") : t("common.refresh") }}
        </button>
      </form>
    </div>

    <KxInlineAlert
      v-if="installSuccessMessage"
      tone="success"
      data-test="skill-catalog-install-success"
    >
      {{ installSuccessMessage }}
    </KxInlineAlert>

    <KxStateBlock
      v-if="store.catalogLoading"
      tone="loading"
      compact
      data-test="skill-catalog-loading-state"
    >
      <span class="spinner" />
      <span class="text-secondary">{{ t("common.loading") }}</span>
    </KxStateBlock>
    <KxStateBlock v-else-if="store.error" tone="error" compact data-test="skill-catalog-error">
      <p class="text-error error">{{ store.error }}</p>
      <button
        class="btn btn-sm"
        type="button"
        data-test="skill-catalog-retry"
        @click="searchCatalog({ force: true })"
      >
        {{ t("common.retry") }}
      </button>
    </KxStateBlock>
    <KxStateBlock
      v-else-if="store.catalogEntries.length === 0"
      tone="empty"
      data-test="skill-catalog-empty"
    >
      <span>{{ t("skills.catalogEmpty") }}</span>
      <button
        class="btn btn-sm"
        type="button"
        data-test="skill-catalog-retry"
        @click="searchCatalog({ force: true })"
      >
        {{ t("common.retry") }}
      </button>
    </KxStateBlock>
    <div v-else class="grid">
      <SkillDiscoverCard
        v-for="entry in store.catalogEntries"
        :key="entry.catalog_id"
        :entry="entry"
        :installing="installingId === entry.package"
        :installed="installedPackages.has(entry.package)"
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

    <ModalDialog
      :open="sourceSettingsOpen"
      :title="t('skills.catalogSourcesTitle')"
      data-test="skill-source-settings-drawer"
      @close="sourceSettingsOpen = false"
    >
      <SkillSourcesSettings />
    </ModalDialog>
  </div>
</template>

<style scoped>
.discover-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.discover-toolbar {
  display: grid;
  gap: 10px;
}

.source-filter {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}

.source-filter .chip {
  padding: 4px 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 14px;
  background: var(--app-card-color);
  cursor: pointer;
  color: var(--app-text-color);
  font-size: 13px;
}

.source-filter .chip.active {
  background: var(--app-primary-color, #18a058);
  color: #fff;
  border-color: var(--app-primary-color, #18a058);
}

.source-filter .settings-icon {
  padding: 4px 8px;
  font-size: 16px;
  margin-left: auto;
}

.discover-search-row {
  display: flex;
  gap: 8px;
  align-items: center;
}

.discover-search-input {
  flex: 1;
  max-width: 360px;
  min-height: 32px;
  padding: 4px 10px;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  font-size: 13px;
  background: var(--app-card-color);
  color: var(--app-text-color);
}

.catalog-state {
  display: grid;
  justify-items: center;
  gap: 10px;
  padding: 24px 0;
  text-align: center;
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
</style>

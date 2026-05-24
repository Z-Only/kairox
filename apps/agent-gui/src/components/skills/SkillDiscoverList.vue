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

type CatalogSort = "relevance" | "name" | "downloads" | "stars" | "security" | "rating";

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
const catalogSort = ref<CatalogSort>("relevance");
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

const catalogSortOptions = computed<{ value: CatalogSort; label: string }[]>(() => [
  { value: "relevance", label: "Relevance" },
  { value: "name", label: t("skills.name") },
  { value: "downloads", label: t("skills.downloads") },
  { value: "stars", label: t("skills.stars") },
  { value: "security", label: t("skills.security") },
  { value: "rating", label: t("skills.rating") }
]);

const sortedCatalogEntries = computed<SkillCatalogEntry[]>(() => {
  if (catalogSort.value === "relevance") return store.catalogEntries;
  return store.catalogEntries
    .map((entry, index) => ({ entry, index }))
    .sort((a, b) => compareCatalogEntries(a.entry, b.entry, catalogSort.value) || a.index - b.index)
    .map(({ entry }) => entry);
});

function compareCatalogEntries(
  a: SkillCatalogEntry,
  b: SkillCatalogEntry,
  sort: CatalogSort
): number {
  switch (sort) {
    case "name":
      return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    case "downloads":
      return compareNumberDesc(a.install_count, b.install_count);
    case "stars":
      return compareNumberDesc(a.github_stars, b.github_stars);
    case "security":
      return compareNumberDesc(a.security_score, b.security_score);
    case "rating":
      return compareNumberDesc(a.rating, b.rating);
    case "relevance":
      return 0;
  }
}

function compareNumberDesc(a: number | null | undefined, b: number | null | undefined): number {
  const left = a ?? Number.NEGATIVE_INFINITY;
  const right = b ?? Number.NEGATIVE_INFINITY;
  if (left === right) return 0;
  return right - left;
}

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
    <SettingsFilterBar :aria-label="t('skills.tabDiscover')">
      <KxChipGroup :aria-label="t('skills.sourceFilter')" data-test="skill-source-filter">
        <KxChipButton
          v-for="source in sourceFilters"
          :key="source.id ?? 'all'"
          size="compact"
          :selected="selectedSourceId === source.id"
          :data-test="`skill-source-filter-${source.id ?? 'all'}`"
          @click="selectSource(source.id)"
        >
          {{ source.display_name }}
        </KxChipButton>
        <template #actions>
          <KxIconButton
            :label="t('marketplace.sourceSettingsAria')"
            data-test="skill-source-settings-btn"
            @click="sourceSettingsOpen = true"
          >
            <span aria-hidden="true">⚙</span>
          </KxIconButton>
        </template>
      </KxChipGroup>

      <form role="search" @submit.prevent="searchCatalog()">
        <KxInput
          v-model="searchKeyword"
          type="search"
          :placeholder="t('skills.searchPlaceholder')"
          data-test="skill-catalog-search"
          size="compact"
        />
        <KxSelect
          v-model="catalogSort"
          aria-label="Skill catalog sort"
          data-test="skill-catalog-sort-select"
          class="catalog-sort-select"
          size="compact"
        >
          <option v-for="option in catalogSortOptions" :key="option.value" :value="option.value">
            {{ option.label }}
          </option>
        </KxSelect>
        <KxToolbarAction
          variant="primary"
          data-test="skill-catalog-search-btn"
          @click="searchCatalog()"
        >
          {{ t("common.search") }}
        </KxToolbarAction>
        <KxToolbarAction
          data-test="skill-catalog-refresh"
          :disabled="store.catalogLoading"
          @click="refreshCatalog"
        >
          {{ store.catalogLoading ? t("skills.refreshing") : t("common.refresh") }}
        </KxToolbarAction>
      </form>
    </SettingsFilterBar>

    <KxInlineAlert
      v-if="installSuccessMessage"
      tone="success"
      data-test="skill-catalog-install-success"
    >
      {{ installSuccessMessage }}
    </KxInlineAlert>

    <SettingsState
      v-if="store.catalogLoading"
      tone="loading"
      data-test="skill-catalog-loading-state"
    >
      {{ t("common.loading") }}
    </SettingsState>
    <SettingsState v-else-if="store.error" tone="error" data-test="skill-catalog-error">
      {{ store.error }}
      <template #actions>
        <KxInlineAction data-test="skill-catalog-retry" @click="searchCatalog({ force: true })">
          {{ t("common.retry") }}
        </KxInlineAction>
      </template>
    </SettingsState>
    <SettingsState
      v-else-if="store.catalogEntries.length === 0"
      tone="empty"
      data-test="skill-catalog-empty"
    >
      {{ t("skills.catalogEmpty") }}
      <template #actions>
        <KxInlineAction data-test="skill-catalog-retry" @click="searchCatalog({ force: true })">
          {{ t("common.retry") }}
        </KxInlineAction>
      </template>
    </SettingsState>
    <div v-else class="grid">
      <SkillDiscoverCard
        v-for="entry in sortedCatalogEntries"
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

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 12px;
}

.catalog-sort-select {
  flex: 0 0 150px;
}
</style>

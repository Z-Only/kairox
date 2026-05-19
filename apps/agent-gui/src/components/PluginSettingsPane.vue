<script setup lang="ts">
import { usePluginsStore } from "@/stores/plugins";
import type { PluginInstallTarget, PluginSettingsView } from "@/generated/commands";

const store = usePluginsStore();
const { t } = useI18n();
const configSource = inject<Ref<"user" | "project">>("configSource");
const activeSubTab = ref<"installed" | "marketplace">("installed");
const search = ref("");
const selectedMarketplaceId = ref<string | null>(null);
const sourceSettingsOpen = ref(false);

const installTarget = computed<PluginInstallTarget>(() =>
  configSource?.value === "project" ? "project" : "user"
);

function slugify(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

function settingsTestId(plugin: PluginSettingsView): string {
  return slugify(plugin.settings_id);
}

async function refreshInstalled(): Promise<void> {
  await store.loadPlugins();
}

async function refreshCatalog(): Promise<void> {
  await Promise.all([
    store.loadSources(),
    store.loadCatalog(selectedMarketplaceId.value, search.value)
  ]);
}

async function installCatalogEntry(marketplaceId: string, name: string): Promise<void> {
  await store.installPlugin(marketplaceId, name, installTarget.value);
}

onMounted(async () => {
  await Promise.all([store.loadPlugins(), store.loadSources()]);
});

watch(activeSubTab, (tab) => {
  if (tab === "marketplace") void refreshCatalog();
});
</script>

<template>
  <section
    class="plugin-settings"
    :aria-label="t('plugins.title')"
    data-test="plugin-settings-pane"
  >
    <KxStateBlock v-if="store.error" tone="error" compact data-test="plugin-error">
      {{ store.error }}
    </KxStateBlock>

    <div class="plugin-sub-tabs" role="tablist" :aria-label="t('plugins.sections')">
      <button
        class="sub-tab-btn"
        role="tab"
        :aria-selected="activeSubTab === 'installed'"
        data-test="plugin-subtab-installed"
        @click="activeSubTab = 'installed'"
      >
        {{ t("plugins.tabInstalled") }}
      </button>
      <button
        class="sub-tab-btn"
        role="tab"
        :aria-selected="activeSubTab === 'marketplace'"
        data-test="plugin-subtab-marketplace"
        @click="activeSubTab = 'marketplace'"
      >
        {{ t("plugins.tabMarketplace") }}
      </button>
    </div>

    <div v-if="activeSubTab === 'installed'" class="plugin-panel">
      <div class="plugin-toolbar">
        <button
          class="btn btn-sm"
          type="button"
          :disabled="store.loading"
          data-test="plugin-refresh"
          @click="refreshInstalled"
        >
          {{ store.loading ? t("plugins.refreshing") : t("common.refresh") }}
        </button>
      </div>

      <KxStateBlock v-if="store.loading" tone="loading" compact data-test="plugin-loading-state">
        {{ t("plugins.loading") }}
      </KxStateBlock>
      <KxStateBlock
        v-else-if="store.plugins.length === 0"
        tone="empty"
        data-test="plugin-empty-state"
      >
        {{ t("plugins.emptyInstalled") }}
      </KxStateBlock>

      <article
        v-for="plugin in store.plugins"
        v-else
        :key="plugin.settings_id"
        class="plugin-row"
        :data-test="`plugin-row-${settingsTestId(plugin)}`"
      >
        <div class="plugin-row__main">
          <div class="plugin-row__title">
            <h4>{{ plugin.name }}</h4>
            <span class="tag">{{ plugin.scope }}</span>
            <span :class="['tag', plugin.enabled ? 'tag-success' : 'tag-warning']">
              {{ plugin.enabled ? t("plugins.enabled") : t("plugins.disabled") }}
            </span>
            <span :class="['tag', plugin.valid ? 'tag-success' : 'tag-error']">
              {{ plugin.valid ? t("plugins.valid") : t("plugins.invalid") }}
            </span>
            <span v-if="!plugin.effective" class="tag tag-warning">
              {{ t("plugins.shadowedBy", { source: plugin.shadowed_by }) }}
            </span>
          </div>
          <p>{{ plugin.description }}</p>
          <dl class="plugin-meta">
            <div>
              <dt>{{ t("plugins.manifest") }}</dt>
              <dd>{{ plugin.manifest_kind }}</dd>
            </div>
            <div>
              <dt>{{ t("plugins.skills") }}</dt>
              <dd>{{ plugin.inventory.skill_count }}</dd>
            </div>
            <div>
              <dt>{{ t("plugins.mcp") }}</dt>
              <dd>{{ plugin.inventory.mcp_server_count }}</dd>
            </div>
            <div>
              <dt>{{ t("plugins.path") }}</dt>
              <dd>{{ plugin.path }}</dd>
            </div>
          </dl>
          <KxInlineAlert v-if="plugin.validation_error" tone="error" compact>
            {{ plugin.validation_error }}
          </KxInlineAlert>
        </div>
        <div class="plugin-actions">
          <button
            class="btn btn-sm"
            type="button"
            :disabled="plugin.scope === 'Builtin' || store.busyPluginId === plugin.settings_id"
            :data-test="`plugin-enabled-${settingsTestId(plugin)}`"
            @click="store.setPluginEnabled(plugin.settings_id, !plugin.enabled)"
          >
            {{ plugin.enabled ? t("plugins.disable") : t("plugins.enable") }}
          </button>
          <button
            class="btn btn-danger btn-sm"
            type="button"
            :disabled="plugin.scope === 'Builtin' || store.busyPluginId === plugin.settings_id"
            :data-test="`plugin-delete-${settingsTestId(plugin)}`"
            @click="store.deletePlugin(plugin.settings_id)"
          >
            {{ t("common.delete") }}
          </button>
        </div>
      </article>
    </div>

    <div v-if="activeSubTab === 'marketplace'" class="plugin-panel">
      <div class="plugin-toolbar">
        <select v-model="selectedMarketplaceId" data-test="plugin-marketplace-filter">
          <option :value="null">{{ t("plugins.allMarketplaces") }}</option>
          <option v-for="source in store.sources" :key="source.id" :value="source.id">
            {{ source.display_name }}
          </option>
        </select>
        <input
          v-model="search"
          type="search"
          :placeholder="t('plugins.searchPlaceholder')"
          data-test="plugin-catalog-search"
          @keyup.enter="refreshCatalog"
        />
        <button
          class="btn btn-sm"
          type="button"
          :disabled="store.catalogLoading"
          data-test="plugin-catalog-refresh"
          @click="refreshCatalog"
        >
          {{ store.catalogLoading ? t("plugins.searching") : t("common.search") }}
        </button>
        <button
          class="btn btn-sm"
          type="button"
          data-test="plugin-source-settings-toggle"
          @click="sourceSettingsOpen = !sourceSettingsOpen"
        >
          {{ t("plugins.sourceSettings") }}
        </button>
      </div>

      <ModalDialog
        :open="sourceSettingsOpen"
        :title="t('plugins.sourceSettings')"
        data-test="plugin-source-settings"
        width="620px"
        @close="sourceSettingsOpen = false"
      >
        <div class="plugin-source-panel">
          <KxStateBlock
            v-if="store.sources.length === 0"
            tone="empty"
            data-test="plugin-source-empty-state"
          >
            {{ t("plugins.emptySources") }}
          </KxStateBlock>
          <article
            v-for="source in store.sources"
            v-else
            :key="source.id"
            class="plugin-row"
            :data-test="`plugin-source-${slugify(source.id)}`"
          >
            <div class="plugin-row__main">
              <div class="plugin-row__title">
                <h4>{{ source.display_name }}</h4>
                <span class="tag">{{ source.id }}</span>
                <span :class="['tag', source.enabled ? 'tag-success' : 'tag-warning']">
                  {{ source.enabled ? t("plugins.enabled") : t("plugins.disabled") }}
                </span>
                <span v-if="source.builtin" class="tag">{{ t("plugins.builtin") }}</span>
              </div>
              <code>{{ source.source }}</code>
            </div>
            <button
              class="btn btn-sm"
              type="button"
              :data-test="`plugin-source-enabled-${slugify(source.id)}`"
              @click="store.setMarketplaceSourceEnabled(source.id, !source.enabled)"
            >
              {{ source.enabled ? t("plugins.disable") : t("plugins.enable") }}
            </button>
          </article>
        </div>

        <template #footer>
          <button
            class="btn btn-sm"
            type="button"
            data-test="plugin-source-settings-close"
            @click="sourceSettingsOpen = false"
          >
            {{ t("common.close") }}
          </button>
        </template>
      </ModalDialog>

      <KxStateBlock
        v-if="store.catalog.length === 0"
        tone="empty"
        data-test="plugin-catalog-empty-state"
      >
        {{ t("plugins.emptyCatalog") }}
      </KxStateBlock>
      <article
        v-for="entry in store.catalog"
        v-else
        :key="`${entry.marketplace_id}:${entry.name}`"
        class="plugin-row"
        data-test="plugin-catalog-card"
      >
        <div class="plugin-row__main">
          <div class="plugin-row__title">
            <h4>{{ entry.name }}</h4>
            <span class="tag">{{ entry.marketplace_id }}</span>
            <span v-if="entry.version" class="tag">{{ entry.version }}</span>
          </div>
          <p>{{ entry.description }}</p>
          <code>{{ entry.source }}</code>
        </div>
        <button
          class="btn btn-primary btn-sm"
          type="button"
          :data-test="`plugin-install-${slugify(entry.marketplace_id)}-${slugify(entry.name)}`"
          @click="installCatalogEntry(entry.marketplace_id, entry.name)"
        >
          {{ t("plugins.install") }}
        </button>
      </article>
    </div>
  </section>
</template>

<style scoped>
.plugin-settings {
  display: flex;
  min-height: 0;
  flex-direction: column;
  gap: 12px;
}
.plugin-sub-tabs,
.plugin-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.sub-tab-btn {
  padding: 6px 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  background: var(--app-card-color);
  color: var(--app-text-color-2);
  cursor: pointer;
}
.sub-tab-btn[aria-selected="true"] {
  color: var(--app-primary-color);
  border-color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 8%, transparent);
}
.plugin-panel {
  min-height: 0;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.plugin-source-panel {
  display: grid;
  gap: 10px;
}
.plugin-row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  padding: 12px;
  background: var(--app-card-color);
}
.plugin-row__main {
  min-width: 0;
  flex: 1;
}
.plugin-row__title {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.plugin-row__title h4 {
  margin: 0;
  font-size: 0.98rem;
}
.plugin-row p {
  margin: 6px 0;
  color: var(--app-text-color-2);
}
.plugin-meta {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 8px;
  margin: 8px 0 0;
}
.plugin-meta dt {
  font-size: 0.72rem;
  color: var(--app-text-color-3);
}
.plugin-meta dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.plugin-actions {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}
.plugin-toolbar input,
.plugin-toolbar select {
  padding: 6px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  background: var(--app-card-color);
  color: var(--app-text-color);
}
code {
  overflow-wrap: anywhere;
}
</style>

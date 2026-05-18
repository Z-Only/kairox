<script setup lang="ts">
import { usePluginsStore } from "@/stores/plugins";
import type { PluginInstallTarget, PluginSettingsView } from "@/generated/commands";

const store = usePluginsStore();
const configSource = inject<Ref<"user" | "project">>("configSource");
const activeSubTab = ref<"installed" | "discover" | "marketplaces">("installed");
const search = ref("");
const selectedMarketplaceId = ref<string | null>(null);

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
  if (tab === "discover") void refreshCatalog();
});
</script>

<template>
  <section class="plugin-settings" aria-label="Plugin settings" data-test="plugin-settings-pane">
    <p v-if="store.error" class="alert alert-error" role="alert" data-test="plugin-error">
      {{ store.error }}
    </p>

    <div class="plugin-sub-tabs" role="tablist" aria-label="Plugin sections">
      <button
        class="sub-tab-btn"
        role="tab"
        :aria-selected="activeSubTab === 'installed'"
        data-test="plugin-subtab-installed"
        @click="activeSubTab = 'installed'"
      >
        Installed
      </button>
      <button
        class="sub-tab-btn"
        role="tab"
        :aria-selected="activeSubTab === 'discover'"
        data-test="plugin-subtab-discover"
        @click="activeSubTab = 'discover'"
      >
        Discover
      </button>
      <button
        class="sub-tab-btn"
        role="tab"
        :aria-selected="activeSubTab === 'marketplaces'"
        data-test="plugin-subtab-marketplaces"
        @click="activeSubTab = 'marketplaces'"
      >
        Marketplaces
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
          {{ store.loading ? "Refreshing" : "Refresh" }}
        </button>
      </div>

      <p v-if="store.loading" class="alert alert-info" role="status">Loading plugins</p>
      <p v-else-if="store.plugins.length === 0" class="empty-state">No plugins installed</p>

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
              {{ plugin.enabled ? "Enabled" : "Disabled" }}
            </span>
            <span :class="['tag', plugin.valid ? 'tag-success' : 'tag-error']">
              {{ plugin.valid ? "Valid" : "Invalid" }}
            </span>
            <span v-if="!plugin.effective" class="tag tag-warning">
              Shadowed by {{ plugin.shadowed_by }}
            </span>
          </div>
          <p>{{ plugin.description }}</p>
          <dl class="plugin-meta">
            <div>
              <dt>Manifest</dt>
              <dd>{{ plugin.manifest_kind }}</dd>
            </div>
            <div>
              <dt>Skills</dt>
              <dd>{{ plugin.inventory.skill_count }}</dd>
            </div>
            <div>
              <dt>MCP</dt>
              <dd>{{ plugin.inventory.mcp_server_count }}</dd>
            </div>
            <div>
              <dt>Path</dt>
              <dd>{{ plugin.path }}</dd>
            </div>
          </dl>
          <p v-if="plugin.validation_error" class="alert alert-error" role="alert">
            {{ plugin.validation_error }}
          </p>
        </div>
        <div class="plugin-actions">
          <button
            class="btn btn-sm"
            type="button"
            :disabled="plugin.scope === 'Builtin' || store.busyPluginId === plugin.settings_id"
            :data-test="`plugin-enabled-${settingsTestId(plugin)}`"
            @click="store.setPluginEnabled(plugin.settings_id, !plugin.enabled)"
          >
            {{ plugin.enabled ? "Disable" : "Enable" }}
          </button>
          <button
            class="btn btn-danger btn-sm"
            type="button"
            :disabled="plugin.scope === 'Builtin' || store.busyPluginId === plugin.settings_id"
            :data-test="`plugin-delete-${settingsTestId(plugin)}`"
            @click="store.deletePlugin(plugin.settings_id)"
          >
            Delete
          </button>
        </div>
      </article>
    </div>

    <div v-if="activeSubTab === 'discover'" class="plugin-panel">
      <div class="plugin-toolbar">
        <select v-model="selectedMarketplaceId" data-test="plugin-marketplace-filter">
          <option :value="null">All marketplaces</option>
          <option v-for="source in store.sources" :key="source.id" :value="source.id">
            {{ source.display_name }}
          </option>
        </select>
        <input
          v-model="search"
          type="search"
          placeholder="Search plugins"
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
          {{ store.catalogLoading ? "Searching" : "Search" }}
        </button>
      </div>

      <p v-if="store.catalog.length === 0" class="empty-state">No marketplace plugins found</p>
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
          Install
        </button>
      </article>
    </div>

    <div v-if="activeSubTab === 'marketplaces'" class="plugin-panel">
      <p v-if="store.sources.length === 0" class="empty-state">No plugin marketplaces configured</p>
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
              {{ source.enabled ? "Enabled" : "Disabled" }}
            </span>
            <span v-if="source.builtin" class="tag">Built-in</span>
          </div>
          <code>{{ source.source }}</code>
        </div>
        <button
          class="btn btn-sm"
          type="button"
          :data-test="`plugin-source-enabled-${slugify(source.id)}`"
          @click="store.setMarketplaceSourceEnabled(source.id, !source.enabled)"
        >
          {{ source.enabled ? "Disable" : "Enable" }}
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

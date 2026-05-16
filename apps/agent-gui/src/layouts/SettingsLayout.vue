<script setup lang="ts">
import { useRoute, useRouter } from "vue-router";
import ConfigSourceBar from "@/components/ConfigSourceBar.vue";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();

const activeTab = computed(() => {
  const segments = route.path.split("/");
  const tab = segments[segments.length - 1];
  return tab && ["general", "instructions", "mcp", "skills", "models", "archive"].includes(tab)
    ? tab
    : "general";
});

const currentSource = ref<"user" | "project">("user");
const currentProjectId = ref<string | undefined>(undefined);

provide("configSource", currentSource);
provide("configProjectId", currentProjectId);

function navigateToTab(tab: string): void {
  router.push(`/settings/${tab}`);
}

function onSourceChange(source: "user" | "project", projectId?: string): void {
  currentSource.value = source;
  currentProjectId.value = projectId;
}
</script>

<template>
  <main class="settings" data-test="view-settings">
    <h1>{{ t("settings.title") }}</h1>

    <div class="tabs" role="tablist" aria-label="Settings sections">
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'general'"
        data-test="settings-tab-general"
        @click="navigateToTab('general')"
      >
        {{ t("settings.general") }}
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'mcp'"
        data-test="settings-tab-mcp"
        @click="navigateToTab('mcp')"
      >
        MCP
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'skills'"
        data-test="settings-tab-skills"
        @click="navigateToTab('skills')"
      >
        Skills
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'models'"
        data-test="settings-tab-models"
        @click="navigateToTab('models')"
      >
        {{ t("models.tabModels") }}
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'instructions'"
        data-test="settings-tab-instructions"
        @click="navigateToTab('instructions')"
      >
        {{ t("settings.instructions") }}
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'archive'"
        data-test="settings-tab-archive"
        @click="navigateToTab('archive')"
      >
        {{ t("settings.archive") }}
      </button>
    </div>

    <div
      v-if="['mcp', 'skills', 'models', 'instructions'].includes(activeTab)"
      class="settings__source-bar"
    >
      <ConfigSourceBar @source-change="onSourceChange" />
    </div>

    <router-view />
  </main>
</template>

<style scoped>
.settings {
  padding: 16px;
  max-width: 960px;
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.settings > :not(.tabs):not(h1) {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.settings > :not(.tabs):not(h1).settings__source-bar {
  flex: none;
  overflow: visible;
}
.tabs {
  display: flex;
  gap: 8px;
  border-bottom: 1px solid var(--app-border-color);
  margin-bottom: 12px;
}
.tab-btn {
  padding: 8px 16px;
  border: none;
  border-bottom: 2px solid transparent;
  border-radius: var(--app-radius-md) var(--app-radius-md) 0 0;
  background: none;
  cursor: pointer;
  font-size: inherit;
  color: var(--app-text-color-2);
}
@media (prefers-reduced-motion: no-preference) {
  .tab-btn {
    transition:
      color 0.2s,
      border-color 0.2s,
      background 0.15s;
  }
}
.tab-btn[aria-selected="true"] {
  color: var(--app-primary-color);
  border-bottom-width: 3px;
  border-bottom-color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 8%, transparent);
}
.tab-btn:hover {
  color: var(--app-text-color);
  background: var(--app-hover-color);
}
.tab-btn:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>

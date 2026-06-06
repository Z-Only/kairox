<script setup lang="ts">
import { useRoute, useRouter } from "vue-router";
import ConfigSourceBar from "@/components/ConfigSourceBar.vue";
import { useSessionStore } from "@/stores/session";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const session = useSessionStore();

const activeTab = computed(() => {
  const segments = route.path.split("/");
  const tab = segments[segments.length - 1];
  return tab &&
    [
      "general",
      "instructions",
      "hooks",
      "mcp",
      "skills",
      "plugins",
      "agents",
      "models",
      "autonomous",
      "archive"
    ].includes(tab)
    ? tab
    : "general";
});

const currentSource = ref<"user" | "project">("user");
const currentProjectId = ref<string | undefined>(undefined);

provide("configSource", currentSource);
provide("configProjectId", currentProjectId);

function syncSourceFromCurrentConversation(): void {
  const projectId = session.currentSessionInfo?.project_id ?? undefined;
  currentSource.value = projectId ? "project" : "user";
  currentProjectId.value = projectId;
}

function navigateToTab(tab: string): void {
  router.push(`/settings/${tab}`);
}

function onSourceChange(source: "user" | "project", projectId?: string): void {
  currentSource.value = source;
  currentProjectId.value = projectId;
}

onMounted(syncSourceFromCurrentConversation);
</script>

<template>
  <main class="settings" data-test="view-settings">
    <header class="settings__header">
      <h1>{{ t("settings.title") }}</h1>
    </header>

    <div class="tabs settings__tabs" role="tablist" aria-label="Settings sections">
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
        {{ t("settings.mcp") }}
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'skills'"
        data-test="settings-tab-skills"
        @click="navigateToTab('skills')"
      >
        {{ t("settings.skills") }}
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'plugins'"
        data-test="settings-tab-plugins"
        @click="navigateToTab('plugins')"
      >
        {{ t("settings.plugins") }}
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'agents'"
        data-test="settings-tab-agents"
        @click="navigateToTab('agents')"
      >
        {{ t("settings.agents") }}
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
        :aria-selected="activeTab === 'hooks'"
        data-test="settings-tab-hooks"
        @click="navigateToTab('hooks')"
      >
        {{ t("settings.hooks") }}
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'autonomous'"
        data-test="settings-tab-autonomous"
        @click="navigateToTab('autonomous')"
      >
        {{ t("settings.autonomous") }}
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
      v-if="
        ['mcp', 'skills', 'plugins', 'agents', 'models', 'instructions', 'hooks'].includes(
          activeTab
        )
      "
      class="settings__source-bar"
    >
      <ConfigSourceBar
        :initial-source="currentSource"
        :initial-project-id="currentProjectId"
        @source-change="onSourceChange"
      />
    </div>

    <router-view />
  </main>
</template>

<style scoped>
.settings {
  width: 100%;
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 20px 24px 18px;
  background:
    linear-gradient(
      180deg,
      color-mix(in srgb, var(--app-panel-color) 72%, transparent) 0,
      transparent 220px
    ),
    var(--app-body-color);
}
.settings__header {
  flex: none;
}
.settings h1 {
  margin: 0;
  color: var(--app-text-color);
  font-size: 24px;
  font-weight: 760;
  line-height: 1.2;
}
.settings > :not(.tabs):not(.settings__header) {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.settings > :not(.tabs):not(.settings__header).settings__source-bar {
  flex: none;
  overflow: visible;
}
.tabs {
  display: flex;
  flex: none;
  gap: 4px;
  border-bottom: 1px solid var(--app-border-color);
  overflow-x: auto;
  scrollbar-width: thin;
}
.tab-btn {
  min-height: 36px;
  padding: 8px 14px;
  border: 1px solid transparent;
  border-bottom: 2px solid transparent;
  border-radius: var(--app-radius-md) var(--app-radius-md) 0 0;
  background: transparent;
  cursor: pointer;
  font-size: inherit;
  font-weight: 600;
  color: var(--app-text-color-2);
  white-space: nowrap;
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
  border-bottom-color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 11%, transparent);
}
.tab-btn:hover {
  color: var(--app-text-color);
  background: var(--app-hover-color);
}
.tab-btn:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
@media (max-width: 720px) {
  .settings {
    padding: 18px 16px;
  }

  .settings h1 {
    font-size: 22px;
  }

  .tabs {
    flex-wrap: wrap;
    overflow-x: visible;
    gap: 2px 4px;
  }

  .tab-btn {
    min-height: 34px;
    padding-inline: 12px;
  }
}
</style>

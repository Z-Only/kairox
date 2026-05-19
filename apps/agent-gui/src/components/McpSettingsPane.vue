<script setup lang="ts">
import { useMcpStore } from "@/stores/mcp";
import { useProjectStore } from "@/stores/project";
import MarketplacePane from "@/components/MarketplacePane.vue";
import McpServerCard from "@/components/McpServerCard.vue";
import McpServerFormDialog from "@/components/McpServerFormDialog.vue";
import SettingsCardList from "@/components/ui/SettingsCardList.vue";

const { t } = useI18n();
const mcp = useMcpStore();
const projectStore = useProjectStore();
const activeSubTab = ref<"installed" | "marketplace">("installed");
const addServerDialogOpen = ref(false);
const addServerMode = ref<"git" | "manual">("manual");
const addServerDropdownOpen = ref(false);

const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

watch(
  [() => configSource?.value, () => configProjectId?.value],
  async () => {
    await mcp.refreshInstalledServers(configSource?.value === "project" ? "project" : null);
  },
  { immediate: true }
);

async function refreshInstalledServers(): Promise<void> {
  await mcp.refreshInstalledServers(configSource?.value === "project" ? "project" : null, {
    forceTools: true
  });
}

// Auto-check health when switching to installed tab
watch(activeSubTab, async (tab) => {
  if (tab === "installed") {
    await mcp.checkAllHealth();
  }
});

function openAddServerDialog(mode: "git" | "manual"): void {
  addServerMode.value = mode;
  addServerDropdownOpen.value = false;
  addServerDialogOpen.value = true;
}

function closeAddServerDialog(): void {
  addServerDialogOpen.value = false;
}
</script>

<template>
  <section class="mcp-settings" aria-label="MCP settings" data-test="mcp-settings-pane">
    <SettingsState v-if="mcp.settingsError" tone="error" data-test="mcp-page-error">
      {{ mcp.settingsError }}
    </SettingsState>

    <SettingsSubtabs aria-label="MCP sections">
      <button
        class="sub-tab-btn"
        role="tab"
        :aria-selected="activeSubTab === 'installed'"
        data-test="mcp-subtab-installed"
        @click="activeSubTab = 'installed'"
      >
        {{ t("mcp.tabInstalled") }}
      </button>
      <button
        class="sub-tab-btn"
        role="tab"
        :aria-selected="activeSubTab === 'marketplace'"
        data-test="mcp-subtab-marketplace"
        @click="activeSubTab = 'marketplace'"
      >
        {{ t("mcp.tabMarketplace") }}
      </button>
    </SettingsSubtabs>

    <section
      v-if="activeSubTab === 'installed'"
      class="mcp-settings__installed"
      data-test="mcp-installed-servers"
    >
      <SettingsToolbar :aria-label="t('mcp.tabInstalled')">
        <button
          class="btn btn-sm"
          type="button"
          :disabled="mcp.configFileOpening"
          data-test="mcp-open-config"
          @click="mcp.openConfigFile()"
        >
          {{ mcp.configFileOpening ? t("mcp.opening") : t("mcp.openConfigFile") }}
        </button>
        <button
          class="btn btn-sm"
          type="button"
          :disabled="mcp.settingsLoading"
          data-test="mcp-refresh-all"
          @click="refreshInstalledServers()"
        >
          {{ mcp.settingsLoading ? t("common.loading") : t("mcp.refreshAll") }}
        </button>
        <KxDropdownMenu
          v-model:open="addServerDropdownOpen"
          content-data-test="mcp-add-server-menu"
          align="end"
        >
          <template #trigger>
            <KxIconButton
              :label="t('mcp.addServer')"
              :title="t('mcp.addServer')"
              data-test="mcp-add-server-btn"
            >
              <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                <path d="M9.25 3h1.5v6.25H17v1.5h-6.25V17h-1.5v-6.25H3v-1.5h6.25V3Z" />
              </svg>
            </KxIconButton>
          </template>
          <template #content>
            <button
              class="kx-dropdown-item"
              type="button"
              data-test="mcp-add-server-manual"
              @click="openAddServerDialog('manual')"
            >
              {{ t("mcp.addManual") }}
            </button>
            <button
              class="kx-dropdown-item"
              type="button"
              data-test="mcp-add-server-git"
              @click="openAddServerDialog('git')"
            >
              {{ t("mcp.addGitInstall") }}
            </button>
          </template>
        </KxDropdownMenu>
      </SettingsToolbar>

      <div class="mcp-settings__body">
        <SettingsState v-if="mcp.settingsLoading" tone="loading" data-test="mcp-loading-state">
          {{ t("mcp.loading") }}
        </SettingsState>
        <SettingsState
          v-else-if="mcp.effectiveServers.length === 0"
          tone="empty"
          data-test="mcp-settings-empty-state"
        >
          {{ t("mcp.noServers") }}
        </SettingsState>

        <SettingsCardList
          v-else
          :aria-label="t('mcp.tabInstalled')"
          data-test="mcp-server-list"
          class="mcp-settings__list"
          :scroll="false"
        >
          <McpServerCard
            v-for="server in mcp.effectiveServers"
            :key="server.value.id"
            :server="server"
          />
        </SettingsCardList>
      </div>
    </section>

    <div v-if="activeSubTab === 'marketplace'" class="mcp-settings__marketplace">
      <MarketplacePane />
    </div>

    <McpServerFormDialog
      :open="addServerDialogOpen"
      :mode="addServerMode"
      @close="closeAddServerDialog"
    />
  </section>
</template>

<style scoped>
.mcp-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow: hidden;
}

/* Wrapper for marketplace tab content — constrain to available height */
.mcp-settings__marketplace {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* Wrapper for installed tab content — toolbar + scrollable list */
.mcp-settings__installed {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.mcp-settings__body {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.mcp-settings button:focus-visible {
  outline: 2px solid var(--app-primary-color, #3b82f6);
  outline-offset: 2px;
}
</style>

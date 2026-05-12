<script setup lang="ts">
import { useMcpStore } from "@/stores/mcp";
import type { McpServerSettingsView } from "@/generated/commands";
import MarketplacePane from "@/components/MarketplacePane.vue";

const { t } = useI18n();
const mcp = useMcpStore();
const activeSubTab = ref<"installed" | "marketplace">("installed");
const addServerDialogOpen = ref(false);
const addServerMode = ref<"git" | "manual">("manual");
const addServerDropdownOpen = ref(false);
const serverName = ref("");
const serverDescription = ref("");
const transport = ref<"stdio" | "sse">("stdio");
const stdioCommand = ref("");
const stdioArgs = ref("");
const sseUrl = ref("");
const busyServerId = ref<string | null>(null);

onMounted(() => {
  void mcp.fetchSettingsServers();
});

function formatTools(server: McpServerSettingsView): string {
  return server.tool_count === null
    ? t("mcp.toolsUnknown")
    : t("mcp.toolsCount", { count: server.tool_count });
}

function resetForm(): void {
  serverName.value = "";
  serverDescription.value = "";
  transport.value = "stdio";
  stdioCommand.value = "";
  stdioArgs.value = "";
  sseUrl.value = "";
}

function openAddServerDialog(mode: "git" | "manual"): void {
  addServerMode.value = mode;
  addServerDropdownOpen.value = false;
  resetForm();
  addServerDialogOpen.value = true;
}

function closeAddServerDialog(): void {
  addServerDialogOpen.value = false;
  resetForm();
}

function parseArgs(argsText: string): string[] {
  return argsText
    .split(/\s+/)
    .map((arg) => arg.trim())
    .filter(Boolean);
}

async function saveServer(): Promise<void> {
  const trimmedName = serverName.value.trim();
  if (!trimmedName) {
    return;
  }

  const savedServer = await mcp.saveServerSettings({
    name: trimmedName,
    transport:
      transport.value === "stdio"
        ? {
            transport: "stdio",
            command: stdioCommand.value.trim(),
            args: parseArgs(stdioArgs.value),
            env: {}
          }
        : {
            transport: "sse",
            url: sseUrl.value.trim(),
            headers: {}
          },
    enabled: true,
    description: serverDescription.value.trim() || null
  });

  if (savedServer) {
    closeAddServerDialog();
  }
}

async function runServerAction(serverId: string, action: () => Promise<void>): Promise<void> {
  busyServerId.value = serverId;
  try {
    await action();
    await mcp.fetchSettingsServers();
  } finally {
    busyServerId.value = null;
  }
}
</script>

<template>
  <section class="mcp-settings" aria-label="MCP settings" data-test="mcp-settings-pane">
    <p v-if="mcp.settingsError" class="alert alert-error" role="alert" data-test="mcp-page-error">
      {{ mcp.settingsError }}
    </p>

    <div class="mcp-sub-tabs" role="tablist" aria-label="MCP sections">
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
    </div>

    <section v-if="activeSubTab === 'installed'" data-test="mcp-installed-servers">
      <div class="mcp-toolbar">
        <button
          class="btn"
          type="button"
          :disabled="mcp.configFolderOpening"
          data-test="mcp-open-config"
          @click="mcp.openConfigFile()"
        >
          {{ mcp.configFolderOpening ? t("mcp.opening") : t("mcp.openConfigFolder") }}
        </button>
        <button
          class="btn"
          type="button"
          :disabled="mcp.settingsLoading"
          data-test="mcp-refresh-all"
          @click="mcp.fetchSettingsServers()"
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
      </div>

      <p v-if="mcp.settingsLoading" class="alert alert-info" role="status">
        {{ t("mcp.loading") }}
      </p>
      <p v-else-if="mcp.settingsServers.length === 0" class="empty-state">
        {{ t("mcp.noServers") }}
      </p>

      <div v-else class="mcp-settings__list" role="list" aria-label="Configured MCP servers">
        <article
          v-for="server in mcp.settingsServers"
          :key="server.id"
          class="card mcp-settings__server"
          role="listitem"
          :data-test="`mcp-server-row-${server.id}`"
        >
          <div class="card-body mcp-settings__server-body">
            <div class="mcp-settings__server-main">
              <h3>{{ server.name }}</h3>
              <p>{{ server.description || t("mcp.noDescription") }}</p>
              <div class="mcp-settings__tags" aria-label="Server metadata">
                <span class="tag">{{ server.transport }}</span>
                <span class="tag">{{ formatTools(server) }}</span>
                <span :class="['tag', server.enabled ? 'tag-success' : 'tag-warning']">
                  {{ server.enabled ? t("mcp.enabled") : t("mcp.disabled") }}
                </span>
                <span :class="['tag', server.trusted ? 'tag-success' : 'tag-warning']">
                  {{ server.trusted ? t("mcp.trusted") : t("mcp.untrusted") }}
                </span>
              </div>
              <p
                v-if="server.last_error"
                class="alert alert-error"
                role="alert"
                :data-test="`mcp-row-error-${server.id}`"
              >
                {{ server.last_error }}
              </p>
            </div>

            <div class="mcp-settings__actions" aria-label="Server actions">
              <button
                class="btn btn-sm"
                type="button"
                :disabled="busyServerId === server.id"
                :data-test="`mcp-refresh-tools-${server.id}`"
                @click="runServerAction(server.id, () => mcp.refreshTools(server.id))"
              >
                {{ busyServerId === server.id ? t("common.loading") : t("mcp.refreshTools") }}
              </button>
              <button
                class="btn btn-sm"
                type="button"
                :disabled="busyServerId === server.id"
                :data-test="`mcp-enable-${server.id}`"
                @click="
                  runServerAction(server.id, () => mcp.setServerEnabled(server.id, !server.enabled))
                "
              >
                {{ server.enabled ? t("mcp.disable") : t("mcp.enable") }}
              </button>
              <button
                class="btn btn-sm"
                type="button"
                :disabled="busyServerId === server.id"
                :data-test="`mcp-trust-${server.id}`"
                @click="
                  runServerAction(server.id, () =>
                    server.trusted ? mcp.revokeTrust(server.id) : mcp.trustServer(server.id)
                  )
                "
              >
                {{ server.trusted ? t("mcp.revokeTrust") : t("mcp.trust") }}
              </button>
              <button
                class="btn btn-danger btn-sm"
                type="button"
                :disabled="!server.writable || busyServerId === server.id"
                :data-test="`mcp-delete-${server.id}`"
                @click="runServerAction(server.id, () => mcp.deleteServerSettings(server.id))"
              >
                {{ t("common.delete") }}
              </button>
            </div>
          </div>
        </article>
      </div>
    </section>

    <MarketplacePane v-if="activeSubTab === 'marketplace'" />

    <ModalDialog
      :open="addServerDialogOpen"
      :title="addServerMode === 'git' ? t('mcp.dialogGitTitle') : t('mcp.dialogManualTitle')"
      :description="addServerMode === 'git' ? t('mcp.dialogGitDesc') : t('mcp.dialogManualDesc')"
      data-test="mcp-add-server-dialog"
      @close="closeAddServerDialog"
    >
      <form class="mcp-settings__form" data-test="mcp-save" @submit.prevent="saveServer">
        <label for="mcp-server-name">{{ t("mcp.serverName") }}</label>
        <input id="mcp-server-name" v-model="serverName" data-test="mcp-form-name" required />

        <template v-if="addServerMode === 'git'">
          <label for="mcp-server-git-url">{{ t("mcp.gitUrl") }}</label>
          <input
            id="mcp-server-git-url"
            v-model="stdioCommand"
            data-test="mcp-form-git-url"
            placeholder="https://github.com/..."
          />
        </template>

        <template v-if="addServerMode === 'manual'">
          <label for="mcp-server-description">{{ t("mcp.description") }}</label>
          <input
            id="mcp-server-description"
            v-model="serverDescription"
            data-test="mcp-form-description"
          />

          <fieldset class="mcp-settings__fieldset">
            <legend>{{ t("mcp.transport") }}</legend>
            <label>
              <input v-model="transport" type="radio" value="stdio" data-test="mcp-form-stdio" />
              stdio
            </label>
            <label>
              <input v-model="transport" type="radio" value="sse" data-test="mcp-form-sse" />
              SSE
            </label>
          </fieldset>

          <template v-if="transport === 'stdio'">
            <label for="mcp-server-command">{{ t("mcp.command") }}</label>
            <input id="mcp-server-command" v-model="stdioCommand" data-test="mcp-form-command" />
            <label for="mcp-server-args">{{ t("mcp.arguments") }}</label>
            <input id="mcp-server-args" v-model="stdioArgs" data-test="mcp-form-args" />
          </template>
          <template v-else>
            <label for="mcp-server-url">{{ t("mcp.sseUrl") }}</label>
            <input id="mcp-server-url" v-model="sseUrl" type="url" data-test="mcp-form-url" />
          </template>
        </template>
      </form>

      <template #footer>
        <button class="btn" type="button" @click="closeAddServerDialog">
          {{ t("common.cancel") }}
        </button>
        <button
          class="btn btn-primary"
          type="submit"
          :disabled="mcp.settingsLoading || !serverName.trim()"
          data-test="mcp-save-button"
          @click="saveServer"
        >
          {{ mcp.settingsLoading ? t("mcp.saving") : t("mcp.saveServer") }}
        </button>
      </template>
    </ModalDialog>
  </section>
</template>

<style scoped>
.mcp-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.mcp-settings__header,
.mcp-settings__server-body {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  justify-content: space-between;
}

.mcp-sub-tabs {
  display: flex;
  gap: 4px;
  border-bottom: 1px solid var(--app-border-color, #e0e0e0);
}

.sub-tab-btn {
  padding: 6px 14px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: 13px;
  color: var(--app-text-color-2, #6b7280);
  border-bottom: 2px solid transparent;
  transition:
    color 0.2s,
    border-color 0.2s;
}

.sub-tab-btn[aria-selected="true"] {
  color: var(--app-primary-color, #18a058);
  border-bottom-color: var(--app-primary-color, #18a058);
}

.sub-tab-btn:hover {
  color: var(--app-primary-color, #18a058);
}

.sub-tab-btn:focus-visible {
  outline: 2px solid var(--app-primary-color, #3b82f6);
  outline-offset: 2px;
}

.mcp-toolbar {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 12px;
}

.mcp-settings__server h3 {
  margin: 0 0 4px;
}

.mcp-settings__tags,
.mcp-settings__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.mcp-settings__list {
  display: grid;
  gap: 12px;
}

.mcp-settings__form {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 0;
}

.mcp-settings__form label + input {
  min-height: 36px;
  padding: 6px 10px;
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: var(--app-card-color, #fff);
  color: var(--app-text-color, #111827);
  width: 100%;
  box-sizing: border-box;
}

.mcp-settings__form label + input:focus,
.mcp-settings button:focus-visible {
  outline: 2px solid var(--app-primary-color, #3b82f6);
  outline-offset: 2px;
}

.mcp-settings__fieldset {
  display: flex;
  gap: 12px;
  padding: 0;
  border: 0;
}

.mcp-settings__server-main {
  min-width: 0;
  display: grid;
  gap: 8px;
}

.mcp-settings__actions {
  justify-content: flex-end;
}
</style>

<script setup lang="ts">
import { useMcpStore } from "@/stores/mcp";
import type { EffectiveMcpServerView, McpServerSettingsView } from "@/generated/commands";
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

const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

watch(
  [() => configSource?.value, () => configProjectId?.value],
  async () => {
    await mcp.fetchSettingsServers(configSource?.value === "project" ? "project" : null);
    await mcp.fetchEffectiveServers();
  },
  { immediate: true }
);

function formatTools(server: McpServerSettingsView | EffectiveMcpServerView): string {
  const toolCount = "value" in server ? server.value.tool_count : server.tool_count;
  return toolCount === null ? t("mcp.toolsUnknown") : t("mcp.toolsCount", { count: toolCount });
}

function testButtonLabel(serverId: string): string {
  if (mcp.testingConnectivity.has(serverId)) return t("mcp.testChecking");
  const result = mcp.connectivityResults[serverId];
  if (!result) return t("mcp.testConnectivity");
  if (result.status === "connected") {
    return t("mcp.testConnected", { count: result.tool_count });
  }
  return t("mcp.testFailed", { reason: result.reason });
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
    await mcp.fetchEffectiveServers();
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

    <section
      v-if="activeSubTab === 'installed'"
      class="mcp-settings__installed"
      data-test="mcp-installed-servers"
    >
      <div class="mcp-toolbar">
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

      <div class="mcp-settings__body">
        <p v-if="mcp.settingsLoading" class="alert alert-info" role="status">
          {{ t("mcp.loading") }}
        </p>
        <p v-else-if="mcp.effectiveServers.length === 0" class="empty-state">
          {{ t("mcp.noServers") }}
        </p>

        <div v-else class="mcp-settings__list" role="list" aria-label="Configured MCP servers">
          <article
            v-for="server in mcp.effectiveServers"
            :key="server.value.id"
            class="card mcp-settings__server"
            role="listitem"
            :data-test="`mcp-server-row-${server.value.id}`"
          >
            <div class="card-body mcp-settings__server-body">
              <div class="mcp-settings__server-main">
                <h3>{{ server.value.name }}</h3>
                <p>{{ server.value.description || t("mcp.noDescription") }}</p>
                <div class="server__tags" aria-label="Server metadata">
                  <span
                    class="tag tag--source"
                    :class="`tag--source-${server.source.toLowerCase()}`"
                  >
                    {{ server.source }}
                  </span>
                  <span v-if="server.overrides" class="tag tag--override">
                    {{ t("mcp.overrides", { source: server.overrides }) }}
                  </span>
                  <span v-if="server.disabledBy" class="tag tag--disabled-by">
                    {{ t("mcp.disabledBy", { source: server.disabledBy }) }}
                  </span>
                  <span class="tag">{{ server.value.transport }}</span>
                  <span class="tag">{{ formatTools(server) }}</span>
                  <span :class="['tag', server.enabled ? 'tag-success' : 'tag-warning']">
                    {{ server.enabled ? t("mcp.enabled") : t("mcp.disabled") }}
                  </span>
                  <span :class="['tag', server.value.trusted ? 'tag-success' : 'tag-warning']">
                    {{ server.value.trusted ? t("mcp.trusted") : t("mcp.untrusted") }}
                  </span>
                </div>
                <p
                  v-if="server.value.last_error"
                  class="alert alert-error"
                  role="alert"
                  :data-test="`mcp-row-error-${server.value.id}`"
                >
                  {{ server.value.last_error }}
                </p>
              </div>

              <div class="mcp-settings__actions" aria-label="Server actions">
                <button
                  class="btn btn-sm"
                  type="button"
                  :disabled="busyServerId === server.value.id"
                  :data-test="`mcp-refresh-tools-${server.value.id}`"
                  @click="runServerAction(server.value.id, () => mcp.refreshTools(server.value.id))"
                >
                  {{
                    busyServerId === server.value.id ? t("common.loading") : t("mcp.refreshTools")
                  }}
                </button>
                <button
                  class="btn btn-sm"
                  type="button"
                  :disabled="busyServerId === server.value.id"
                  :data-test="`mcp-enable-${server.value.id}`"
                  @click="
                    runServerAction(server.value.id, () =>
                      mcp.setServerEnabled(server.value.id, !server.enabled)
                    )
                  "
                >
                  {{ server.enabled ? t("mcp.disable") : t("mcp.enable") }}
                </button>
                <button
                  class="btn btn-sm"
                  type="button"
                  :disabled="busyServerId === server.value.id"
                  :data-test="`mcp-trust-${server.value.id}`"
                  @click="
                    runServerAction(server.value.id, () =>
                      server.value.trusted
                        ? mcp.revokeTrust(server.value.id)
                        : mcp.trustServer(server.value.id)
                    )
                  "
                >
                  {{ server.value.trusted ? t("mcp.revokeTrust") : t("mcp.trust") }}
                </button>
                <button
                  class="btn btn-sm"
                  type="button"
                  :disabled="
                    mcp.testingConnectivity.has(server.value.id) || busyServerId === server.value.id
                  "
                  :data-test="`mcp-test-connectivity-${server.value.id}`"
                  @click="mcp.testConnectivity(server.value.id)"
                >
                  {{ testButtonLabel(server.value.id) }}
                </button>
                <button
                  class="btn btn-danger btn-sm"
                  type="button"
                  :disabled="!server.writable || busyServerId === server.value.id"
                  :data-test="`mcp-delete-${server.value.id}`"
                  @click="
                    runServerAction(server.value.id, () =>
                      mcp.deleteServerSettings(server.value.id)
                    )
                  "
                >
                  {{ t("common.delete") }}
                </button>
              </div>
            </div>
          </article>
        </div>
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
  overflow: hidden;
}

.mcp-settings__header,
.mcp-settings__server-body {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  justify-content: space-between;
}

/* Wrapper for installed tab content — toolbar + scrollable list */
.mcp-settings__installed {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 0;
}

.mcp-settings__body {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
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
  flex: none;
}

.mcp-settings__server h3 {
  margin: 0 0 4px;
}

.mcp-settings__tags,
.server__tags,
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

/* Source tags for effective (unified) view */
.tag--source {
  font-weight: 600;
}

.tag--source-builtin {
  background: var(--color-muted);
  color: var(--color-text-muted);
}

.tag--source-user {
  background: var(--color-secondary-light);
  color: var(--color-secondary);
}

.tag--source-project {
  background: var(--color-primary-light);
  color: var(--color-primary);
}

.tag--source-local {
  background: var(--color-accent-light, var(--color-primary-light));
  color: var(--color-accent, var(--color-primary));
}

.tag--override {
  background: var(--color-warning-light);
  color: var(--color-warning);
}

.tag--disabled-by {
  background: var(--color-danger-light);
  color: var(--color-danger);
}
</style>

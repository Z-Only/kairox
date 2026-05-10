<script setup lang="ts">
import MarketplacePane from "@/components/MarketplacePane.vue";
import { useMcpStore } from "@/stores/mcp";
import type { McpServerSettingsView } from "@/generated/commands";

const mcp = useMcpStore();
const activeSubTab = ref<"servers" | "marketplace">("servers");
const addServerOpen = ref(false);
const installMode = ref<"catalog" | "manual">("catalog");
const searchQuery = ref("");
const serverName = ref("");
const serverDescription = ref("");
const transport = ref<"stdio" | "sse">("stdio");
const stdioCommand = ref("");
const stdioArgs = ref("");
const sseUrl = ref("");
const busyServerId = ref<string | null>(null);

const filteredServers = computed(() => {
  const normalizedQuery = searchQuery.value.trim().toLowerCase();
  if (!normalizedQuery) {
    return mcp.settingsServers;
  }

  return mcp.settingsServers.filter((server) => {
    const searchableText = [server.name, server.id, server.transport, server.description ?? ""]
      .join(" ")
      .toLowerCase();
    return searchableText.includes(normalizedQuery);
  });
});

onMounted(() => {
  void mcp.fetchSettingsServers();
});

function formatTools(server: McpServerSettingsView): string {
  return server.tool_count === null ? "tools unknown" : `${server.tool_count} tools`;
}

function resetForm(): void {
  serverName.value = "";
  serverDescription.value = "";
  transport.value = "stdio";
  stdioCommand.value = "";
  stdioArgs.value = "";
  sseUrl.value = "";
}

function openAddServerPanel(): void {
  installMode.value = "catalog";
  addServerOpen.value = true;
}

function closeAddServerPanel(): void {
  addServerOpen.value = false;
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
    closeAddServerPanel();
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
  <section class="mcp-settings" aria-labelledby="mcp-settings-title" data-test="mcp-settings-pane">
    <header class="mcp-settings__header">
      <div>
        <h2 id="mcp-settings-title">MCP settings</h2>
        <p>Manage configured MCP servers first, then browse the embedded marketplace.</p>
      </div>
      <button
        class="btn"
        type="button"
        :disabled="mcp.settingsLoading"
        data-test="mcp-open-config"
        @click="mcp.openConfigFile()"
      >
        {{ mcp.settingsLoading ? "Opening…" : "Open config file" }}
      </button>
    </header>

    <div class="mcp-settings__tabs" role="tablist" aria-label="MCP settings sections">
      <button
        class="btn btn-ghost"
        type="button"
        role="tab"
        :aria-selected="activeSubTab === 'servers'"
        data-test="mcp-subtab-servers"
        @click="activeSubTab = 'servers'"
      >
        Servers
      </button>
      <button
        class="btn btn-ghost"
        type="button"
        role="tab"
        :aria-selected="activeSubTab === 'marketplace'"
        data-test="mcp-subtab-marketplace"
        @click="activeSubTab = 'marketplace'"
      >
        Marketplace
      </button>
    </div>

    <p v-if="mcp.settingsError" class="alert alert-error" role="alert" data-test="mcp-page-error">
      {{ mcp.settingsError }}
    </p>

    <div v-if="activeSubTab === 'servers'" role="tabpanel" aria-label="MCP servers">
      <section class="card mcp-installed-servers" data-test="mcp-installed-servers">
        <div class="mcp-section-header">
          <div>
            <h3>Configured servers</h3>
            <p>Review installed MCP servers, runtime status, trust state, and row actions.</p>
          </div>
          <button
            class="btn btn-primary"
            type="button"
            data-test="mcp-add-server-btn"
            @click="openAddServerPanel"
          >
            Add server
          </button>
        </div>

        <div class="mcp-settings__controls">
          <label for="mcp-server-search">Search servers</label>
          <input
            id="mcp-server-search"
            v-model="searchQuery"
            type="search"
            data-test="mcp-search"
            placeholder="Filter by name, id, transport, or description"
          />
        </div>

        <p v-if="mcp.settingsLoading" class="alert alert-info" role="status">
          Loading MCP servers…
        </p>
        <p v-else-if="filteredServers.length === 0" class="empty-state">No MCP servers match.</p>

        <div v-else class="mcp-settings__list" role="list" aria-label="Configured MCP servers">
          <article
            v-for="server in filteredServers"
            :key="server.id"
            class="card mcp-settings__server"
            role="listitem"
            :data-test="`mcp-server-row-${server.id}`"
          >
            <div class="card-body mcp-settings__server-body">
              <div class="mcp-settings__server-main">
                <h3>{{ server.name }}</h3>
                <p>{{ server.description || "No description provided." }}</p>
                <div class="mcp-settings__tags" aria-label="Server metadata">
                  <span class="tag">{{ server.transport }}</span>
                  <span class="tag">{{ server.runtime_status }}</span>
                  <span class="tag">{{ formatTools(server) }}</span>
                  <span :class="['tag', server.enabled ? 'tag-success' : 'tag-warning']">
                    {{ server.enabled ? "Enabled" : "Disabled" }}
                  </span>
                  <span :class="['tag', server.trusted ? 'tag-success' : 'tag-warning']">
                    {{ server.trusted ? "Trusted" : "Untrusted" }}
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
                  :data-test="`mcp-enable-${server.id}`"
                  @click="
                    runServerAction(server.id, () =>
                      mcp.setServerEnabled(server.id, !server.enabled)
                    )
                  "
                >
                  {{ server.enabled ? "Disable" : "Enable" }}
                </button>
                <button
                  class="btn btn-sm"
                  type="button"
                  :disabled="busyServerId === server.id"
                  :data-test="`mcp-start-stop-${server.id}`"
                  @click="
                    runServerAction(server.id, () =>
                      server.runtime_status === 'running'
                        ? mcp.stopServer(server.id)
                        : mcp.startServer(server.id)
                    )
                  "
                >
                  {{ server.runtime_status === "running" ? "Stop" : "Start" }}
                </button>
                <button
                  class="btn btn-sm"
                  type="button"
                  :disabled="busyServerId === server.id"
                  :data-test="`mcp-refresh-${server.id}`"
                  @click="runServerAction(server.id, () => mcp.refreshTools(server.id))"
                >
                  Refresh tools
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
                  {{ server.trusted ? "Revoke trust" : "Trust" }}
                </button>
                <button
                  class="btn btn-sm"
                  type="button"
                  disabled
                  title="Editing existing MCP servers requires full transport details from the backend."
                  :data-test="`mcp-edit-${server.id}`"
                >
                  Edit
                </button>
                <button
                  class="btn btn-danger btn-sm"
                  type="button"
                  :disabled="!server.writable || busyServerId === server.id"
                  :data-test="`mcp-delete-${server.id}`"
                  @click="runServerAction(server.id, () => mcp.deleteServerSettings(server.id))"
                >
                  Delete
                </button>
              </div>
            </div>
          </article>
        </div>
      </section>

      <section
        v-if="addServerOpen"
        class="card mcp-add-server-panel"
        data-test="mcp-add-server-panel"
      >
        <div class="mcp-section-header">
          <div>
            <h3>Add server</h3>
            <p>Install from the catalog or add a custom stdio/SSE server manually.</p>
          </div>
          <button class="btn btn-ghost" type="button" @click="closeAddServerPanel">Close</button>
        </div>

        <div class="segmented" role="tablist" aria-label="Add server mode">
          <button
            class="segmented-btn"
            type="button"
            role="tab"
            :aria-selected="installMode === 'catalog'"
            data-test="mcp-install-mode-catalog"
            @click="installMode = 'catalog'"
          >
            Catalog / Git
          </button>
          <button
            class="segmented-btn"
            type="button"
            role="tab"
            :aria-selected="installMode === 'manual'"
            data-test="mcp-install-mode-manual"
            @click="installMode = 'manual'"
          >
            Manual
          </button>
        </div>

        <MarketplacePane v-if="installMode === 'catalog'" />

        <form v-else class="mcp-settings__form" data-test="mcp-save" @submit.prevent="saveServer">
          <div class="card-body mcp-settings__form-body">
            <label for="mcp-server-name">Server name</label>
            <input id="mcp-server-name" v-model="serverName" data-test="mcp-form-name" required />

            <label for="mcp-server-description">Description</label>
            <input
              id="mcp-server-description"
              v-model="serverDescription"
              data-test="mcp-form-description"
            />

            <fieldset class="mcp-settings__fieldset">
              <legend>Transport</legend>
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
              <label for="mcp-server-command">Command</label>
              <input id="mcp-server-command" v-model="stdioCommand" data-test="mcp-form-command" />
              <label for="mcp-server-args">Arguments</label>
              <input id="mcp-server-args" v-model="stdioArgs" data-test="mcp-form-args" />
            </template>
            <template v-else>
              <label for="mcp-server-url">SSE URL</label>
              <input id="mcp-server-url" v-model="sseUrl" type="url" data-test="mcp-form-url" />
            </template>

            <div class="mcp-settings__form-actions">
              <button
                class="btn btn-primary"
                type="submit"
                :disabled="mcp.settingsLoading || !serverName.trim()"
                data-test="mcp-save-button"
              >
                {{ mcp.settingsLoading ? "Saving…" : "Save server" }}
              </button>
              <button class="btn" type="button" data-test="mcp-reset-form" @click="resetForm">
                Reset
              </button>
            </div>
          </div>
        </form>
      </section>
    </div>

    <div v-if="activeSubTab === 'marketplace'" role="tabpanel" aria-label="MCP marketplace">
      <MarketplacePane />
    </div>
  </section>
</template>

<style scoped>
.mcp-settings {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.mcp-settings__header,
.mcp-settings__server-body,
.mcp-settings__form-actions {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  justify-content: space-between;
}

.mcp-settings__header h2,
.mcp-settings__server h3 {
  margin: 0 0 4px;
}

.mcp-settings__header p,
.mcp-settings__server p {
  margin: 0;
  color: var(--app-text-color-2, #6b7280);
}

.mcp-settings__tabs,
.mcp-settings__tags,
.mcp-settings__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.mcp-settings__controls,
.mcp-settings__form-body,
.mcp-settings__list {
  display: grid;
  gap: 12px;
}

.mcp-settings__controls,
.mcp-settings__form {
  margin-bottom: 16px;
}

.mcp-installed-servers,
.mcp-add-server-panel {
  padding: 12px;
}

.mcp-section-header {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 12px;
}

.mcp-section-header h3,
.mcp-section-header p {
  margin: 0;
}

.mcp-section-header p {
  color: var(--app-text-color-2, #6b7280);
}

.mcp-settings__controls {
  padding: 0;
}

.segmented {
  display: inline-flex;
  gap: 4px;
  padding: 4px;
  margin-bottom: 12px;
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 10px;
  background: var(--app-color-fill-2, rgba(0, 0, 0, 0.04));
}

.segmented-btn {
  min-height: 36px;
  padding: 6px 12px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--app-text-color-2, #6b7280);
  cursor: pointer;
}

.segmented-btn[aria-selected="true"] {
  background: var(--app-card-color, #fff);
  color: var(--app-text-color, #111827);
  box-shadow: 0 1px 3px rgba(15, 23, 42, 0.12);
}

.mcp-settings__form-body input,
.mcp-settings__controls input {
  min-height: 36px;
  padding: 6px 10px;
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: var(--app-card-color, #fff);
  color: var(--app-text-color, #111827);
}

.mcp-settings__form-body input:focus,
.mcp-settings__controls input:focus,
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

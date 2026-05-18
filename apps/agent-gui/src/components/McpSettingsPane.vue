<script setup lang="ts">
import { useMcpStore } from "@/stores/mcp";
import { useProjectStore } from "@/stores/project";
import type {
  ConfigScope,
  EffectiveMcpServerView,
  McpContentBlockResponse,
  McpServerSettingsView
} from "@/generated/commands";
import MarketplacePane from "@/components/MarketplacePane.vue";
import ScopeSelector from "@/components/ScopeSelector.vue";

const { t } = useI18n();
const mcp = useMcpStore();
const projectStore = useProjectStore();
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
const installTarget = ref<ConfigScope>("User");

const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

/** When a project is selected, resolve its root path for config writes. */
const currentProjectRoot = computed(() => {
  const pid = configProjectId?.value;
  if (!pid) return undefined;
  const project = projectStore.activeProjects.find((p) => p.projectId === pid);
  return project?.rootPath;
});

/** Whether we are in a project-config context where project-scope disable is relevant. */
const isProjectScope = computed(() => configSource?.value === "project");

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

function formatTools(server: McpServerSettingsView | EffectiveMcpServerView): string {
  const toolCount = "value" in server ? server.value.tool_count : server.tool_count;
  return toolCount === null ? t("mcp.toolsUnknown") : t("mcp.toolsCount", { count: toolCount });
}

function testButtonLabel(serverId: string): string {
  if (mcp.testingConnectivity.has(serverId)) return t("mcp.testChecking");
  return t("mcp.testConnectivity");
}

function healthLabel(serverId: string): string {
  if (mcp.checkingHealth.has(serverId)) return t("mcp.checkingHealth");
  const h = mcp.serverHealth[serverId];
  if (!h) return "";
  return h.healthy ? t("mcp.healthy") : t("mcp.unhealthy");
}

function healthClass(serverId: string): string {
  const h = mcp.serverHealth[serverId];
  if (!h) return "";
  return h.healthy ? "tag-success" : "tag-danger";
}

function serverToolCount(serverId: string): number {
  return mcp.serverHealth[serverId]?.tools?.length ?? 0;
}

// Resource & prompt accordion state (local, not in store)
const expandedResources = ref<Set<string>>(new Set());
const expandedPrompts = ref<Set<string>>(new Set());

function toggleResourcesExpanded(serverId: string): void {
  const next = new Set(expandedResources.value);
  if (next.has(serverId)) {
    next.delete(serverId);
  } else {
    next.add(serverId);
    mcp.fetchResources(serverId);
  }
  expandedResources.value = next;
}

function togglePromptsExpanded(serverId: string): void {
  const next = new Set(expandedPrompts.value);
  if (next.has(serverId)) {
    next.delete(serverId);
  } else {
    next.add(serverId);
    mcp.fetchPrompts(serverId);
  }
  expandedPrompts.value = next;
}

function resourceCount(serverId: string): number {
  return mcp.serverResources[serverId]?.length ?? 0;
}

function promptCount(serverId: string): number {
  return mcp.serverPrompts[serverId]?.length ?? 0;
}

function isResourceExpanded(serverId: string, uri: string): boolean {
  return mcp.expandedResourceUri[serverId] === uri;
}

async function handleResourceClick(serverId: string, uri: string): Promise<void> {
  if (isResourceExpanded(serverId, uri)) {
    mcp.toggleResourceExpand(serverId, uri);
    return;
  }
  await mcp.readResource(serverId, uri);
  mcp.toggleResourceExpand(serverId, uri);
}

function resourceContentBlocks(serverId: string, uri: string): McpContentBlockResponse[] {
  return mcp.resourceContentCache[`${serverId}:${uri}`] ?? [];
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

function canDisableAtScope(server: EffectiveMcpServerView): boolean {
  return (
    isProjectScope.value &&
    currentProjectRoot.value !== undefined &&
    server.source === "User" &&
    !server.disabledBy
  );
}

function canEnableAtScope(server: EffectiveMcpServerView): boolean {
  return (
    isProjectScope.value &&
    currentProjectRoot.value !== undefined &&
    server.disabledBy === "Project"
  );
}

async function disableAtScope(serverId: string): Promise<void> {
  const root = currentProjectRoot.value;
  if (!root) return;
  await runServerAction(serverId, () => mcp.disableServerAtScope(serverId, root));
}

async function enableAtScope(serverId: string): Promise<void> {
  const root = currentProjectRoot.value;
  if (!root) return;
  await runServerAction(serverId, () => mcp.enableServerAtScope(serverId, root));
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
                  <span :class="['tag', server.enabled ? 'tag-success' : 'tag-warning']">
                    {{ server.enabled ? t("mcp.enabled") : t("mcp.disabled") }}
                  </span>
                  <span :class="['tag', server.value.trusted ? 'tag-success' : 'tag-warning']">
                    {{ server.value.trusted ? t("mcp.trusted") : t("mcp.untrusted") }}
                  </span>
                  <span
                    v-if="server.source === 'builtin' && !server.value.verified"
                    class="tag tag--unverified"
                  >
                    {{ t("mcp.unverified") }}
                  </span>
                  <!-- Health badge (non-builtin servers only) -->
                  <span
                    v-if="server.value.transport !== 'builtin' && healthLabel(server.value.id)"
                    :class="['tag', healthClass(server.value.id)]"
                    :data-test="`mcp-health-${server.value.id}`"
                  >
                    {{ healthLabel(server.value.id) }}
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
                  v-if="server.value.transport !== 'builtin'"
                  class="btn btn-sm"
                  type="button"
                  :disabled="
                    mcp.checkingHealth.has(server.value.id) || busyServerId === server.value.id
                  "
                  :data-test="`mcp-recheck-${server.value.id}`"
                  @click="mcp.checkHealth(server.value.id)"
                >
                  {{
                    mcp.checkingHealth.has(server.value.id)
                      ? t("mcp.checkingHealth")
                      : t("mcp.recheckHealth")
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
                  v-if="canDisableAtScope(server)"
                  class="btn btn-sm btn-warning"
                  type="button"
                  :disabled="busyServerId === server.value.id"
                  :data-test="`mcp-disable-scope-${server.value.id}`"
                  @click="disableAtScope(server.value.id)"
                >
                  {{ t("mcp.disableInProject") }}
                </button>
                <button
                  v-if="canEnableAtScope(server)"
                  class="btn btn-sm btn-success"
                  type="button"
                  :disabled="busyServerId === server.value.id"
                  :data-test="`mcp-enable-scope-${server.value.id}`"
                  @click="enableAtScope(server.value.id)"
                >
                  {{ t("mcp.enableInProject") }}
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

              <!-- Collapsible tool list at card bottom (non-builtin servers only) -->
              <div
                v-if="server.value.transport !== 'builtin' && serverToolCount(server.value.id) > 0"
                class="mcp-settings__tools"
                :data-test="`mcp-tools-${server.value.id}`"
              >
                <button
                  class="mcp-settings__tools-toggle"
                  type="button"
                  :aria-expanded="mcp.expandedServers.has(server.value.id)"
                  :data-test="`mcp-tools-toggle-${server.value.id}`"
                  @click="mcp.toggleExpanded(server.value.id)"
                >
                  <span class="toggle-icon">{{
                    mcp.expandedServers.has(server.value.id) ? "▼" : "▶"
                  }}</span>
                  {{ t("mcp.toolCount", { count: serverToolCount(server.value.id) }) }}
                </button>

                <div
                  v-if="mcp.expandedServers.has(server.value.id)"
                  class="mcp-settings__tools-list"
                >
                  <button
                    v-for="tool in mcp.serverHealth[server.value.id]?.tools ?? []"
                    :key="tool.name"
                    class="mcp-settings__tool-btn"
                    :class="{
                      'mcp-settings__tool-btn--enabled': !mcp.isToolDisabled(
                        server.value.id,
                        tool.name
                      ),
                      'mcp-settings__tool-btn--disabled': mcp.isToolDisabled(
                        server.value.id,
                        tool.name
                      )
                    }"
                    :title="tool.description ?? tool.name"
                    :data-test="`mcp-tool-${server.value.id}-${tool.name}`"
                    @click="
                      mcp.setToolDisabled(
                        server.value.id,
                        tool.name,
                        !mcp.isToolDisabled(server.value.id, tool.name)
                      )
                    "
                  >
                    {{ tool.name }}
                  </button>
                </div>
              </div>

              <!-- Collapsible resource list at card bottom (non-builtin servers only) -->
              <div
                v-if="server.value.transport !== 'builtin'"
                class="mcp-settings__resources"
                :data-test="`mcp-resources-${server.value.id}`"
              >
                <button
                  class="mcp-settings__tools-toggle"
                  type="button"
                  :aria-expanded="expandedResources.has(server.value.id)"
                  :data-test="`mcp-resources-toggle-${server.value.id}`"
                  @click="toggleResourcesExpanded(server.value.id)"
                >
                  <span class="toggle-icon">{{
                    expandedResources.has(server.value.id) ? "▼" : "▶"
                  }}</span>
                  <template v-if="mcp.loadingResources.has(server.value.id)">
                    {{ t("mcp.loadingResources") }}
                  </template>
                  <template v-else>
                    {{ t("mcp.resourceCount", { count: resourceCount(server.value.id) }) }}
                  </template>
                </button>

                <div
                  v-if="expandedResources.has(server.value.id)"
                  class="mcp-settings__resources-list"
                >
                  <p
                    v-if="mcp.resourcesError[server.value.id]"
                    class="alert alert-error"
                    role="alert"
                  >
                    {{ mcp.resourcesError[server.value.id] }}
                  </p>
                  <p
                    v-else-if="
                      resourceCount(server.value.id) === 0 &&
                      !mcp.loadingResources.has(server.value.id)
                    "
                    class="empty-state"
                  >
                    {{ t("mcp.noResources") }}
                  </p>
                  <template
                    v-for="resource in mcp.serverResources[server.value.id] ?? []"
                    :key="resource.uri"
                  >
                    <button
                      class="mcp-settings__resource-row"
                      type="button"
                      :aria-expanded="isResourceExpanded(server.value.id, resource.uri)"
                      :data-test="`mcp-resource-${server.value.id}-${resource.name}`"
                      @click="handleResourceClick(server.value.id, resource.uri)"
                    >
                      <span class="toggle-icon">{{
                        isResourceExpanded(server.value.id, resource.uri) ? "▼" : "▶"
                      }}</span>
                      <span class="resource-name">{{ resource.name }}</span>
                      <span class="resource-uri">{{ resource.uri }}</span>
                      <span v-if="resource.mime_type" class="tag tag--mime">{{
                        resource.mime_type
                      }}</span>
                    </button>
                    <div
                      v-if="isResourceExpanded(server.value.id, resource.uri)"
                      class="mcp-settings__resource-content"
                      :data-test="`mcp-resource-content-${server.value.id}-${resource.name}`"
                    >
                      <div
                        v-for="(block, blockIdx) in resourceContentBlocks(
                          server.value.id,
                          resource.uri
                        )"
                        :key="blockIdx"
                        class="mcp-settings__content-block"
                      >
                        <pre v-if="block.type === 'text'" class="content-block__text">{{
                          block.text
                        }}</pre>
                        <img
                          v-else-if="block.type === 'image'"
                          :src="`data:${block.mime_type};base64,${block.data}`"
                          class="content-block__image"
                          :alt="resource.name"
                        />
                        <a
                          v-else-if="block.type === 'resource'"
                          class="content-block__link"
                          :href="block.uri"
                          >{{ block.name || block.uri }}</a
                        >
                      </div>
                    </div>
                  </template>
                </div>
              </div>

              <!-- Collapsible prompt list at card bottom (non-builtin servers only) -->
              <div
                v-if="server.value.transport !== 'builtin'"
                class="mcp-settings__prompts"
                :data-test="`mcp-prompts-${server.value.id}`"
              >
                <button
                  class="mcp-settings__tools-toggle"
                  type="button"
                  :aria-expanded="expandedPrompts.has(server.value.id)"
                  :data-test="`mcp-prompts-toggle-${server.value.id}`"
                  @click="togglePromptsExpanded(server.value.id)"
                >
                  <span class="toggle-icon">{{
                    expandedPrompts.has(server.value.id) ? "▼" : "▶"
                  }}</span>
                  <template v-if="mcp.loadingPrompts.has(server.value.id)">
                    {{ t("mcp.loadingPrompts") }}
                  </template>
                  <template v-else>
                    {{ t("mcp.promptCount", { count: promptCount(server.value.id) }) }}
                  </template>
                </button>

                <div v-if="expandedPrompts.has(server.value.id)" class="mcp-settings__prompts-list">
                  <p
                    v-if="mcp.promptsError[server.value.id]"
                    class="alert alert-error"
                    role="alert"
                  >
                    {{ mcp.promptsError[server.value.id] }}
                  </p>
                  <p
                    v-else-if="
                      promptCount(server.value.id) === 0 && !mcp.loadingPrompts.has(server.value.id)
                    "
                    class="empty-state"
                  >
                    {{ t("mcp.noPrompts") }}
                  </p>
                  <div
                    v-for="prompt in mcp.serverPrompts[server.value.id] ?? []"
                    :key="prompt.name"
                    class="mcp-settings__prompt-row"
                    :data-test="`mcp-prompt-${server.value.id}-${prompt.name}`"
                  >
                    <span class="prompt-name">{{ prompt.name }}</span>
                    <span class="tag tag--mime">{{
                      t("mcp.argumentsCount", { count: prompt.argument_count })
                    }}</span>
                    <span v-if="prompt.description" class="prompt-desc">{{
                      prompt.description
                    }}</span>
                  </div>
                </div>
              </div>
            </div>
          </article>
        </div>
      </div>
    </section>

    <div v-if="activeSubTab === 'marketplace'" class="mcp-settings__marketplace">
      <MarketplacePane />
    </div>

    <ModalDialog
      :open="addServerDialogOpen"
      :title="addServerMode === 'git' ? t('mcp.dialogGitTitle') : t('mcp.dialogManualTitle')"
      :description="addServerMode === 'git' ? t('mcp.dialogGitDesc') : t('mcp.dialogManualDesc')"
      data-test="mcp-add-server-dialog"
      @close="closeAddServerDialog"
    >
      <form class="mcp-settings__form" data-test="mcp-save" @submit.prevent="saveServer">
        <ScopeSelector v-model="installTarget" :show-local="true" />

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
  flex-wrap: wrap;
  gap: 12px;
  align-items: flex-start;
  justify-content: space-between;
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

.tag--unverified {
  background: var(--color-warning-light);
  color: var(--color-warning);
}

/* ── Health badge ── */
.tag-danger {
  background: var(--color-danger-light, #fee2e2);
  color: var(--color-danger, #dc2626);
}

/* ── Collapsible tool list ── */
.mcp-settings__tools {
  width: 100%;
  margin-top: 4px;
  border-top: 1px solid var(--app-border-color, #e0e0e0);
  padding-top: 8px;
}

.mcp-settings__tools-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: 12px;
  color: var(--app-text-color-2, #6b7280);
  border-radius: 4px;
}

.mcp-settings__tools-toggle:hover {
  background: var(--app-hover-color, #f3f4f6);
}

.toggle-icon {
  font-size: 10px;
}

.mcp-settings__tools-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 6px;
}

.mcp-settings__tool-btn {
  display: inline-flex;
  align-items: center;
  padding: 4px 10px;
  border: 1px solid transparent;
  border-radius: 14px;
  cursor: pointer;
  font-size: 12px;
  font-family: monospace;
  font-weight: 500;
  transition:
    background-color 0.15s,
    border-color 0.15s,
    opacity 0.15s;
}

.mcp-settings__tool-btn--enabled {
  background: var(--color-success-light, #d1fae5);
  color: var(--color-success, #059669);
  border-color: var(--color-success, #059669);
}

.mcp-settings__tool-btn--enabled:hover {
  background: var(--color-danger-light, #fee2e2);
  color: var(--color-danger, #dc2626);
  border-color: var(--color-danger, #dc2626);
}

.mcp-settings__tool-btn--disabled {
  background: var(--color-muted-light, #f3f4f6);
  color: var(--color-text-muted, #9ca3af);
  border-color: var(--app-border-color, #d7d7d7);
}

.mcp-settings__tool-btn--disabled:hover {
  background: var(--color-success-light, #d1fae5);
  color: var(--color-success, #059669);
  border-color: var(--color-success, #059669);
}

/* ── Resource & prompt lists ── */
.mcp-settings__resources,
.mcp-settings__prompts {
  width: 100%;
  margin-top: 4px;
  border-top: 1px solid var(--app-border-color, #e0e0e0);
  padding-top: 8px;
}

.mcp-settings__resources-list,
.mcp-settings__prompts-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 6px;
}

.mcp-settings__resource-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: var(--app-card-color, #fff);
  cursor: pointer;
  font-size: 12px;
  text-align: left;
  width: 100%;
  transition: background-color 0.15s;
}

.mcp-settings__resource-row:hover {
  background: var(--app-hover-color, #f3f4f6);
}

.resource-name {
  font-weight: 600;
  white-space: nowrap;
}

.resource-uri {
  flex: 1;
  color: var(--app-text-color-2, #6b7280);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: monospace;
  font-size: 11px;
}

.tag--mime {
  background: var(--color-muted-light, #f3f4f6);
  color: var(--color-text-muted, #6b7280);
  font-size: 10px;
  text-transform: uppercase;
}

.mcp-settings__resource-content {
  padding: 8px;
  border: 1px solid var(--app-border-color, #e0e0e0);
  border-radius: 6px;
  background: var(--app-bg-color, #f9fafb);
  margin-bottom: 4px;
}

.mcp-settings__content-block {
  max-width: 100%;
}

.content-block__text {
  margin: 0;
  padding: 8px;
  background: #1e1e1e;
  color: #d4d4d4;
  border-radius: 4px;
  font-size: 12px;
  max-height: 300px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-all;
}

.content-block__image {
  max-width: 100%;
  max-height: 400px;
  border-radius: 4px;
}

.content-block__link {
  color: var(--app-primary-color, #18a058);
  font-size: 12px;
  word-break: break-all;
}

.mcp-settings__prompt-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
  background: var(--app-card-color, #fff);
  font-size: 12px;
}

.prompt-name {
  font-weight: 600;
  font-family: monospace;
}

.prompt-desc {
  flex: 1;
  color: var(--app-text-color-2, #6b7280);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>

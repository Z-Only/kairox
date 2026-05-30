<script setup lang="ts">
import { useMcpStore } from "@/stores/mcp";
import { useProjectStore } from "@/stores/project";
import type { EffectiveMcpServerView } from "@/generated/commands";
import McpResourceAccordion from "@/components/McpResourceAccordion.vue";
import McpPromptAccordion from "@/components/McpPromptAccordion.vue";
import SettingsEffectiveAudit from "@/components/ui/SettingsEffectiveAudit.vue";
import SettingsCardItem from "@/components/ui/SettingsCardItem.vue";
import SettingsItemSummary from "@/components/ui/SettingsItemSummary.vue";
import SettingsStatusTag from "@/components/ui/SettingsStatusTag.vue";

type SourceTone = "source-builtin" | "source-user" | "source-project" | "source-local";

const { t } = useI18n();
const mcp = useMcpStore();
const projectStore = useProjectStore();

const props = defineProps<{
  server: EffectiveMcpServerView;
}>();

const configSource = inject<Ref<"user" | "project">>("configSource");
const configProjectId = inject<Ref<string | undefined>>("configProjectId");

const busy = ref(false);

const currentProjectRoot = computed(() => {
  const pid = configProjectId?.value;
  if (!pid) return undefined;
  const project = projectStore.activeProjects.find((p) => p.projectId === pid);
  return project?.rootPath;
});

const isProjectScope = computed(() => configSource?.value === "project");

async function runAction(action: () => Promise<void>): Promise<void> {
  busy.value = true;
  try {
    await action();
    await mcp.fetchSettingsServers();
    await mcp.fetchEffectiveServers();
  } finally {
    busy.value = false;
  }
}

function canDisableAtScope(): boolean {
  return (
    isProjectScope.value &&
    currentProjectRoot.value !== undefined &&
    props.server.source === "User" &&
    !props.server.disabledBy
  );
}

function canEnableAtScope(): boolean {
  return (
    isProjectScope.value &&
    currentProjectRoot.value !== undefined &&
    props.server.disabledBy === "Project"
  );
}

function healthLabel(): string {
  if (mcp.checkingHealth.has(props.server.value.id)) return t("mcp.checkingHealth");
  const h = mcp.serverHealth[props.server.value.id];
  if (!h) return "";
  return h.healthy ? t("mcp.healthy") : t("mcp.unhealthy");
}

function healthTone(): "success" | "error" {
  const h = mcp.serverHealth[props.server.value.id];
  return h?.healthy ? "success" : "error";
}

function connectivityLabel(): string {
  if (mcp.testingConnectivity.has(props.server.value.id)) return t("mcp.testChecking");
  const result = mcp.connectivityResults[props.server.value.id];
  if (!result) return "";
  if (result.status === "connected") {
    return t("mcp.testConnected", { count: result.tool_count });
  }
  return t("mcp.testFailed", { reason: result.reason });
}

function connectivityTone(): "success" | "error" | "warning" {
  if (mcp.testingConnectivity.has(props.server.value.id)) return "warning";
  const result = mcp.connectivityResults[props.server.value.id];
  return result?.status === "connected" ? "success" : "error";
}

function sourceTone(source: string): SourceTone {
  switch (source.toLowerCase()) {
    case "builtin":
      return "source-builtin";
    case "project":
      return "source-project";
    case "local":
      return "source-local";
    default:
      return "source-user";
  }
}

function serverToolCount(): number {
  return mcp.serverHealth[props.server.value.id]?.tools?.length ?? 0;
}
</script>

<template>
  <SettingsCardItem class="mcp-settings__server" :data-test="`mcp-server-row-${server.value.id}`">
    <SettingsItemSummary
      :title="server.value.name"
      :description="server.value.description || t('mcp.noDescription')"
      :tags-label="t('mcp.title')"
    >
      <template #tags>
        <SettingsStatusTag :tone="sourceTone(server.source)">
          {{ server.source }}
        </SettingsStatusTag>
        <SettingsStatusTag v-if="server.overrides" tone="override">
          {{ t("mcp.overrides", { source: server.overrides }) }}
        </SettingsStatusTag>
        <SettingsStatusTag v-if="server.disabledBy" tone="disabled-by">
          {{ t("mcp.disabledBy", { source: server.disabledBy }) }}
        </SettingsStatusTag>
        <SettingsStatusTag>{{ server.value.transport }}</SettingsStatusTag>
        <SettingsStatusTag :tone="server.enabled ? 'success' : 'warning'">
          {{ server.enabled ? t("mcp.enabled") : t("mcp.disabled") }}
        </SettingsStatusTag>
        <SettingsStatusTag :tone="server.value.trusted ? 'success' : 'warning'">
          {{ server.value.trusted ? t("mcp.trusted") : t("mcp.untrusted") }}
        </SettingsStatusTag>
        <SettingsStatusTag
          v-if="server.source === 'builtin' && !server.value.verified"
          tone="warning"
        >
          {{ t("mcp.unverified") }}
        </SettingsStatusTag>
        <SettingsStatusTag
          v-if="server.value.transport !== 'builtin' && healthLabel()"
          :tone="healthTone()"
          :data-test="`mcp-health-${server.value.id}`"
        >
          {{ healthLabel() }}
        </SettingsStatusTag>
        <SettingsStatusTag
          v-if="server.value.transport !== 'builtin' && connectivityLabel()"
          :tone="connectivityTone()"
          :data-test="`mcp-connectivity-${server.value.id}`"
        >
          {{ connectivityLabel() }}
        </SettingsStatusTag>
      </template>
      <SettingsEffectiveAudit
        :source="server.source"
        :source-tone="sourceTone(server.source)"
        :enabled="server.enabled"
        :effective="server.enabled"
        :overrides="server.overrides"
        :disabled-by="server.disabledBy"
        :data-test="`mcp-audit-${server.value.id}`"
      />
      <p
        v-if="server.value.diagnostic_summary"
        class="mcp-settings__diagnostics"
        :data-test="`mcp-diagnostics-${server.value.id}`"
      >
        {{ server.value.diagnostic_summary }}
      </p>
      <KxInlineAlert
        v-if="server.value.last_error"
        tone="error"
        compact
        :data-test="`mcp-row-error-${server.value.id}`"
      >
        {{ server.value.last_error }}
      </KxInlineAlert>
    </SettingsItemSummary>

    <template #actions>
      <KxInlineAction
        v-if="server.value.transport !== 'builtin'"
        :disabled="mcp.checkingHealth.has(server.value.id) || busy"
        :data-test="`mcp-recheck-${server.value.id}`"
        @click="mcp.checkHealth(server.value.id)"
      >
        {{
          mcp.checkingHealth.has(server.value.id) ? t("mcp.checkingHealth") : t("mcp.recheckHealth")
        }}
      </KxInlineAction>
      <KxInlineAction
        v-if="server.value.transport !== 'builtin'"
        :disabled="mcp.testingConnectivity.has(server.value.id) || busy"
        :data-test="`mcp-test-connectivity-${server.value.id}`"
        @click="mcp.testConnectivity(server.value.id)"
      >
        {{
          mcp.testingConnectivity.has(server.value.id)
            ? t("mcp.testChecking")
            : t("mcp.testConnectivity")
        }}
      </KxInlineAction>
      <KxInlineAction
        :disabled="busy"
        :data-test="`mcp-enable-${server.value.id}`"
        @click="runAction(() => mcp.setServerEnabled(server.value.id, !server.enabled))"
      >
        {{ server.enabled ? t("mcp.disable") : t("mcp.enable") }}
      </KxInlineAction>
      <KxInlineAction
        :disabled="busy"
        :data-test="`mcp-trust-${server.value.id}`"
        @click="
          runAction(() =>
            server.value.trusted
              ? mcp.revokeTrust(server.value.id)
              : mcp.trustServer(server.value.id)
          )
        "
      >
        {{ server.value.trusted ? t("mcp.revokeTrust") : t("mcp.trust") }}
      </KxInlineAction>
      <KxInlineAction
        v-if="canDisableAtScope()"
        variant="warning"
        :disabled="busy"
        :data-test="`mcp-disable-scope-${server.value.id}`"
        @click="runAction(() => mcp.disableServerAtScope(server.value.id, currentProjectRoot!))"
      >
        {{ t("mcp.disableInProject") }}
      </KxInlineAction>
      <KxInlineAction
        v-if="canEnableAtScope()"
        variant="success"
        :disabled="busy"
        :data-test="`mcp-enable-scope-${server.value.id}`"
        @click="runAction(() => mcp.enableServerAtScope(server.value.id, currentProjectRoot!))"
      >
        {{ t("mcp.enableInProject") }}
      </KxInlineAction>
      <KxInlineAction
        variant="danger"
        :disabled="!server.writable || busy"
        :data-test="`mcp-delete-${server.value.id}`"
        @click="runAction(() => mcp.deleteServerSettings(server.value.id))"
      >
        {{ t("common.delete") }}
      </KxInlineAction>
    </template>

    <template #details>
      <!-- Collapsible tool list at card bottom (non-builtin servers only) -->
      <div
        v-if="server.value.transport !== 'builtin' && serverToolCount() > 0"
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
          {{ t("mcp.toolCount", { count: serverToolCount() }) }}
        </button>

        <div v-if="mcp.expandedServers.has(server.value.id)" class="mcp-settings__tools-list">
          <button
            v-for="tool in mcp.serverHealth[server.value.id]?.tools ?? []"
            :key="tool.name"
            class="mcp-settings__tool-btn"
            :class="{
              'mcp-settings__tool-btn--enabled': !mcp.isToolDisabled(server.value.id, tool.name),
              'mcp-settings__tool-btn--disabled': mcp.isToolDisabled(server.value.id, tool.name)
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
      <McpResourceAccordion
        v-if="server.value.transport !== 'builtin'"
        :server-id="server.value.id"
      />

      <!-- Collapsible prompt list at card bottom (non-builtin servers only) -->
      <McpPromptAccordion
        v-if="server.value.transport !== 'builtin'"
        :server-id="server.value.id"
      />
    </template>
  </SettingsCardItem>
</template>

<style scoped>
.mcp-settings__diagnostics {
  margin: 0;
  color: var(--app-text-muted);
  font-size: 12px;
  line-height: 1.4;
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
</style>

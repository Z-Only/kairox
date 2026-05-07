<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import { useMcpStore } from "@/stores/mcp";
import { useAgentsStore } from "@/stores/agents";
import type { TraceEntryData } from "../types/trace";

const { t } = useI18n();
const props = defineProps<{ entry: TraceEntryData }>();
const mcp = useMcpStore();
const agents = useAgentsStore();

const isMemory = props.entry.kind === "memory";

/** Detect MCP tools by their "mcp.{server_id}.{tool_name}" format. */
const isMcpTool = computed(() => props.entry.toolId?.startsWith("mcp."));

/** Extract the server ID from an MCP tool ID like "mcp.github.list_repos". */
const mcpServerId = computed(() => {
  if (!isMcpTool.value) return null;
  const parts = props.entry.toolId!.split(".");
  // "mcp.{server_id}.{tool_name}" — server_id may contain dots, but
  // conventionally the second segment is the server ID.
  return parts.length >= 3 ? parts[1] : null;
});

/** Whether this MCP server is already trusted. */
const isServerTrusted = computed(() => {
  if (!mcpServerId.value) return false;
  return mcp.trustedServerIds.includes(mcpServerId.value);
});

/** Checkbox state for "Trust this server". */
const trustChecked = ref(false);

/** The source agent label if available from the entry's rawEvent. */
const sourceAgentLabel = computed(() => {
  if (!props.entry.rawEvent) return null;
  try {
    const event = JSON.parse(props.entry.rawEvent);
    const agentId = event?.source_agent_id;
    if (agentId && agentId !== "agent_system") {
      return agents.agentLabel(agentId);
    }
  } catch {
    // Ignore parse errors
  }
  return null;
});

const alertType = computed<"warning" | "success">(() =>
  isMemory ? "success" : "warning"
);
const allowLabel = computed(() =>
  isMemory ? t("permission.accept") : t("permission.allow")
);
const denyLabel = computed(() =>
  isMemory ? t("permission.reject") : t("permission.deny")
);
const titleLabel = computed(() =>
  isMemory
    ? t("permission.titleMemoryProposed")
    : t("permission.titlePermissionRequired")
);
const iconLabel = computed(() => (isMemory ? "🧠" : "🔑"));

async function allow() {
  try {
    await invoke("resolve_permission", {
      requestId: props.entry.id,
      decision: "grant"
    });
    if (isMcpTool.value && trustChecked.value && mcpServerId.value) {
      await mcp.trustServer(mcpServerId.value);
    }
  } catch (e) {
    console.error("Failed to grant permission:", e);
  }
}

async function deny() {
  try {
    await invoke("resolve_permission", {
      requestId: props.entry.id,
      decision: "deny"
    });
  } catch (e) {
    console.error("Failed to deny permission:", e);
  }
}
</script>

<template>
  <!-- NaiveUI NAlert hosts the whole prompt; .permission-prompt /
       .memory-prompt wrappers are kept as hook classes so existing
       tests and any consumer styling can still target them. -->
  <NAlert
    :class="['permission-prompt', isMemory ? 'memory-prompt' : '']"
    :type="alertType"
    :show-icon="false"
    :bordered="true"
    size="small"
  >
    <NCard size="small" :bordered="false" class="permission-card">
      <NSpace align="start" :size="8" :wrap="false">
        <span class="permission-icon">{{ iconLabel }}</span>
        <div class="permission-body">
          <NSpace align="center" :size="6" class="permission-title-row">
            <NText strong class="permission-title">{{ titleLabel }}</NText>
            <NTag
              v-if="sourceAgentLabel"
              size="small"
              type="info"
              :bordered="false"
              class="permission-agent-badge"
            >
              {{ sourceAgentLabel }}
            </NTag>
          </NSpace>
          <NText depth="2" class="permission-description">
            {{ entry.title }}
          </NText>
          <div v-if="entry.scope" class="permission-meta">
            Scope: {{ entry.scope }}
          </div>
          <div v-if="entry.content" class="permission-meta">
            {{ entry.content }}
          </div>
          <div class="permission-meta">
            {{ isMemory ? "Store" : "Tool" }}: {{ entry.toolId }}
          </div>
          <!-- MCP-specific UI. The wrapper classes (.mcp-permission-info,
               .mcp-trust-check, .mcp-trusted-badge) are kept verbatim so
               permission-prompt tests can still query them after the
               NaiveUI migration. -->
          <div v-if="isMcpTool && mcpServerId" class="mcp-permission-info">
            <div class="mcp-server-label">
              MCP Server: <strong>{{ mcpServerId }}</strong>
              <NTag
                v-if="isServerTrusted"
                size="small"
                type="success"
                :bordered="false"
                class="mcp-trusted-badge"
              >
                ✅ Trusted
              </NTag>
            </div>
            <!-- NCheckbox replaces the previous native <input type="checkbox">
                 so the control follows the surrounding NaiveUI dark-theme
                 palette. The .mcp-trust-check wrapper class is preserved so
                 layout selectors keep working; tests drive the control via
                 [data-test="trust-server-checkbox"] +
                 findComponent({ name: "Checkbox" }) instead of reaching for
                 a raw <input>. -->
            <div v-if="!isServerTrusted" class="mcp-trust-check">
              <NCheckbox
                v-model:checked="trustChecked"
                size="small"
                data-test="trust-server-checkbox"
              >
                Trust this server for future requests
              </NCheckbox>
            </div>
          </div>
        </div>
        <NSpace :size="6" :wrap="false" class="permission-actions">
          <!-- NButton renders an inner <button>; the .btn-allow / .btn-deny
               wrapper classes preserve the existing test selectors. -->
          <NButton type="success" size="small" class="btn-allow" @click="allow">
            {{ allowLabel }}
          </NButton>
          <NButton size="small" class="btn-deny" @click="deny">
            {{ denyLabel }}
          </NButton>
        </NSpace>
      </NSpace>
    </NCard>
  </NAlert>
</template>

<style scoped>
.permission-prompt {
  margin: 4px 0;
}
.permission-card :deep(.n-card__content) {
  padding: 4px 8px;
}
.permission-icon {
  font-size: 16px;
  flex-shrink: 0;
}
.permission-body {
  flex: 1;
  min-width: 0;
}
.permission-title-row {
  margin-bottom: 2px;
}
.permission-title {
  font-size: 12px;
}
.permission-agent-badge {
  font-size: 10px;
}
.permission-description {
  display: block;
  margin: 2px 0 4px;
  font-size: 12px;
}
.permission-meta {
  font-size: 11px;
  color: var(--app-text-color-3, #777);
  margin-top: 2px;
  overflow-wrap: anywhere;
}
.permission-actions {
  flex-shrink: 0;
}
.mcp-permission-info {
  margin-top: 6px;
  padding: 4px 8px;
  background: var(--app-popover-color, #f0f4ff);
  border-radius: 4px;
  border: 1px solid var(--app-border-color, #c8d6f0);
}
.mcp-server-label {
  font-size: 11px;
}
.mcp-trusted-badge {
  margin-left: 6px;
}
.mcp-trust-check {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
  font-size: 11px;
  /* `cursor: pointer` removed (7c review carry-over): after the inner
     control migrated to <NCheckbox>, clicking the wrapper no longer
     toggles the box, so the pointer cursor was misleading. */
}
</style>

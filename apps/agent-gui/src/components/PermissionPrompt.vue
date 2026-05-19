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

const alertType = computed<"warning" | "success">(() => (isMemory ? "success" : "warning"));
const allowLabel = computed(() => (isMemory ? t("permission.accept") : t("permission.allow")));
const denyLabel = computed(() => (isMemory ? t("permission.reject") : t("permission.deny")));
const titleLabel = computed(() =>
  isMemory ? t("permission.titleMemoryProposed") : t("permission.titlePermissionRequired")
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
  <!-- .permission-prompt / .memory-prompt wrappers are kept as hook
       classes so existing tests and any consumer styling can still
       target them. -->
  <div
    :class="['permission-prompt', 'alert', `alert-${alertType}`, isMemory ? 'memory-prompt' : '']"
    data-test="permission-prompt"
  >
    <div class="card permission-card">
      <div class="permission-layout">
        <span class="permission-icon">{{ iconLabel }}</span>
        <div class="permission-body">
          <div class="permission-title-row">
            <strong class="permission-title">{{ titleLabel }}</strong>
            <KxBadge v-if="sourceAgentLabel" class="permission-agent-badge" tone="info">
              {{ sourceAgentLabel }}
            </KxBadge>
          </div>
          <span :style="{ color: 'var(--app-text-color-2)' }" class="permission-description">
            {{ entry.title }}
          </span>
          <div v-if="entry.scope" class="permission-meta">
            {{ t("permission.scopePrefix") }}: {{ entry.scope }}
          </div>
          <div v-if="entry.content" class="permission-meta">
            {{ entry.content }}
          </div>
          <div class="permission-meta">
            {{ isMemory ? t("permission.storeLabel") : t("permission.toolLabel") }}:
            {{ entry.toolId }}
          </div>
          <!-- MCP-specific UI. The wrapper classes (.mcp-permission-info,
               .mcp-trust-check, .mcp-trusted-badge) are kept verbatim so
               permission-prompt tests can still query them. -->
          <div v-if="isMcpTool && mcpServerId" class="mcp-permission-info">
            <div class="mcp-server-label">
              {{ t("permission.mcpServerPrefix") }}: <strong>{{ mcpServerId }}</strong>
              <KxBadge v-if="isServerTrusted" class="mcp-trusted-badge" tone="success">
                ✅ {{ t("permission.mcpTrustedBadge") }}
              </KxBadge>
            </div>
            <!-- The .mcp-trust-check wrapper class is preserved so layout
                 selectors keep working; tests drive the control via
                 [data-test="trust-server-checkbox"] +
                 findComponent({ name: "Checkbox" }) instead of reaching for
                 a raw <input>. -->
            <div v-if="!isServerTrusted" class="mcp-trust-check">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  :checked="trustChecked"
                  data-test="trust-server-checkbox"
                  @change="trustChecked = $event.target.checked"
                />
                {{ t("permission.mcpTrustCheckbox") }}
              </label>
            </div>
          </div>
        </div>
        <div class="permission-actions">
          <KxButton variant="primary" size="xs" data-test="permission-allow" @click="allow">
            {{ allowLabel }}
          </KxButton>
          <KxButton size="xs" data-test="permission-deny" @click="deny">
            {{ denyLabel }}
          </KxButton>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.permission-prompt {
  margin: 4px 0;
}

/* Alert base & variants */
.alert {
  border-radius: 4px;
  border: 1px solid var(--app-border-color, #e0e0e0);
}

.alert-warning {
  background: var(--app-warning-color-suppl, #fff8e6);
  border-color: var(--app-warning-color, #e8b339);
}

.alert-success {
  background: var(--app-success-color-suppl, #edf8ee);
  border-color: var(--app-success-color, #18a058);
}

/* Card replacement */
.permission-card {
  padding: 4px 8px;
}

/* Layout (replaces NSpace align="start" :size="8" :wrap="false") */
.permission-layout {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  flex-wrap: nowrap;
}

.permission-icon {
  font-size: 16px;
  flex-shrink: 0;
}

.permission-body {
  flex: 1;
  min-width: 0;
}

/* Title row (replaces NSpace align="center" :size="6") */
.permission-title-row {
  display: flex;
  align-items: center;
  gap: 6px;
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

/* Actions (replaces NSpace :size="6" :wrap="false") */
.permission-actions {
  display: flex;
  gap: 6px;
  flex-wrap: nowrap;
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
}

/* Checkbox label (replaces NCheckbox) */
.checkbox-label {
  display: flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  font-size: 11px;
}
</style>

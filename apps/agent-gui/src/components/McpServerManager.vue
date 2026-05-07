<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useMcpStore } from "@/stores/mcp";

const { t } = useI18n();
const mcp = useMcpStore();
const emit = defineEmits<{ close: [] }>();

// Emoji + i18n text are split so the user-visible label is translatable
// while the status indicator emoji stays consistent across locales.
function statusEmoji(status: string): string {
  switch (status) {
    case "running":
      return "🟢";
    case "starting":
      return "🟡";
    case "failed":
      return "🔴";
    default:
      return "⚪";
  }
}

function statusText(status: string): string {
  switch (status) {
    case "running":
      return t("mcp.statusRunning");
    case "starting":
      return t("mcp.statusStarting");
    case "failed":
      return t("mcp.statusFailed");
    default:
      return t("mcp.statusStopped");
  }
}

function statusTagType(
  status: string
): "success" | "warning" | "error" | "default" {
  switch (status) {
    case "running":
      return "success";
    case "starting":
      return "warning";
    case "failed":
      return "error";
    default:
      return "default";
  }
}

const trustedSet = computed(() => new Set(mcp.trustedServerIds));
</script>

<template>
  <!-- The popover wrapper class (.mcp-manager) is preserved so
       StatusBar's positioning + the existing tests keep working.
       NCard provides the chrome (header, body, divider) while
       NList renders the per-server rows. The .mcp-close-btn /
       .mcp-server-actions class hooks are also kept since the
       existing test suite drives the UI through them. -->
  <div class="mcp-manager">
    <NCard
      size="small"
      :bordered="true"
      class="mcp-manager-card"
      content-style="padding: 0;"
    >
      <template #header>
        <NText strong>MCP Servers</NText>
      </template>
      <template #header-extra>
        <NButton
          quaternary
          size="tiny"
          class="mcp-close-btn"
          @click="emit('close')"
        >
          ✕
        </NButton>
      </template>

      <NEmpty
        v-if="mcp.servers.length === 0"
        size="small"
        class="mcp-empty-wrap"
      >
        <template #default>
          <p class="mcp-empty">No MCP servers configured</p>
        </template>
      </NEmpty>

      <NList v-else hoverable :bordered="false" class="mcp-manager-list">
        <NListItem v-for="server in mcp.servers" :key="server.id">
          <div class="mcp-server-item">
            <div class="mcp-server-info">
              <NText strong class="mcp-server-name">{{ server.id }}</NText>
              <NTag
                size="small"
                :type="statusTagType(server.status)"
                :bordered="false"
                class="mcp-server-status"
              >
                {{ statusEmoji(server.status) }} {{ statusText(server.status) }}
              </NTag>
              <NTag
                v-if="trustedSet.has(server.id)"
                size="small"
                type="success"
                :bordered="false"
                class="mcp-trusted"
              >
                ✅ Trusted
              </NTag>
              <NTag
                v-else-if="server.status === 'running'"
                size="small"
                type="warning"
                :bordered="false"
                class="mcp-untrusted"
              >
                ⚠️ Not trusted
              </NTag>
            </div>

            <NText
              v-if="server.status === 'failed' && server.error"
              type="error"
              class="mcp-server-error"
            >
              {{ server.error }}
            </NText>

            <!-- The .mcp-server-actions wrapper + per-button order is
                 preserved verbatim so the existing test suite (which
                 picks "the first button") keeps targeting Start/Stop/
                 Restart correctly. -->
            <NSpace :size="4" class="mcp-server-actions" :wrap="true">
              <NButton
                v-if="server.status === 'stopped'"
                size="tiny"
                @click="mcp.startServer(server.id)"
              >
                Start
              </NButton>
              <NButton
                v-if="server.status === 'running'"
                size="tiny"
                @click="mcp.stopServer(server.id)"
              >
                Stop
              </NButton>
              <NButton
                v-if="server.status === 'failed'"
                size="tiny"
                @click="mcp.startServer(server.id)"
              >
                Restart
              </NButton>
              <NButton
                v-if="server.status === 'running' && !trustedSet.has(server.id)"
                size="tiny"
                @click="mcp.trustServer(server.id)"
              >
                Trust
              </NButton>
              <NButton
                v-if="trustedSet.has(server.id)"
                size="tiny"
                @click="mcp.revokeTrust(server.id)"
              >
                Revoke
              </NButton>
              <NButton
                v-if="server.status === 'running'"
                size="tiny"
                @click="mcp.refreshTools(server.id)"
              >
                Refresh
              </NButton>
            </NSpace>

            <NText
              v-if="server.status === 'running' && server.tool_count"
              depth="3"
              class="mcp-server-meta"
            >
              {{ server.tool_count }} tools
            </NText>
          </div>
        </NListItem>
      </NList>
    </NCard>
  </div>
</template>

<style scoped>
.mcp-manager {
  position: absolute;
  right: 0;
  bottom: 100%;
  width: 320px;
  max-height: 400px;
  overflow-y: auto;
  z-index: 100;
}
.mcp-manager-card {
  box-shadow: 0 -2px 8px rgba(0, 0, 0, 0.12);
}
.mcp-close-btn {
  font-size: 14px;
  line-height: 1;
}
.mcp-empty-wrap {
  padding: 16px 0;
}
.mcp-empty {
  font-size: 12px;
  margin: 0;
  text-align: center;
}
.mcp-manager-list :deep(.n-list-item) {
  padding: 8px 12px;
}
.mcp-server-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.mcp-server-info {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.mcp-server-name {
  font-size: 12px;
}
.mcp-server-status,
.mcp-trusted,
.mcp-untrusted {
  font-size: 10px;
}
.mcp-server-error {
  font-size: 11px;
  overflow-wrap: anywhere;
}
.mcp-server-meta {
  font-size: 11px;
}
</style>

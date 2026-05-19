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

function statusTagType(status: string): "success" | "warning" | "error" | "default" {
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
       The card provides the chrome (header, body, divider) while
       the list renders the per-server rows. The .mcp-close-btn /
       .mcp-server-actions hooks are also kept since the existing
       test suite drives the UI through them. -->
  <div class="mcp-manager" data-test="mcp-manager">
    <div class="card mcp-manager-card">
      <div class="card-header">
        <span class="card-title"><strong>MCP Servers</strong></span>
        <KxIconButton
          class="mcp-close-btn"
          label="Close MCP servers"
          data-test="mcp-close-btn"
          @click="emit('close')"
        >
          ✕
        </KxIconButton>
      </div>

      <SettingsState v-if="mcp.servers.length === 0" tone="empty" data-test="mcp-empty-state">
        No MCP servers configured
      </SettingsState>

      <ul v-else class="list mcp-manager-list">
        <li v-for="server in mcp.servers" :key="server.id" class="list-item">
          <div class="mcp-server-item" data-test="mcp-server-item">
            <div class="mcp-server-info">
              <span class="mcp-server-name" data-test="mcp-server-name"
                ><strong>{{ server.id }}</strong></span
              >
              <span
                class="tag mcp-server-status"
                :class="`tag-${statusTagType(server.status)}`"
                data-test="mcp-server-status"
              >
                {{ statusEmoji(server.status) }} {{ statusText(server.status) }}
              </span>
              <span v-if="trustedSet.has(server.id)" class="tag tag-success mcp-trusted">
                ✅ Trusted
              </span>
              <span v-else-if="server.status === 'running'" class="tag tag-warning mcp-untrusted">
                ⚠️ Not trusted
              </span>
            </div>

            <span
              v-if="server.status === 'failed' && server.error"
              class="text-error mcp-server-error"
              data-test="mcp-server-error"
            >
              {{ server.error }}
            </span>

            <!-- The .mcp-server-actions wrapper + per-button order is
                 preserved verbatim so the existing test suite (which
                 picks "the first button") keeps targeting Start/Stop/
                 Restart correctly. -->
            <div class="flex-wrap mcp-server-actions">
              <KxButton
                v-if="server.status === 'stopped'"
                size="xs"
                data-test="mcp-start-btn"
                @click="mcp.startServer(server.id)"
              >
                Start
              </KxButton>
              <KxButton
                v-if="server.status === 'running'"
                size="xs"
                data-test="mcp-stop-btn"
                @click="mcp.stopServer(server.id)"
              >
                Stop
              </KxButton>
              <KxButton
                v-if="server.status === 'failed'"
                size="xs"
                data-test="mcp-restart-btn"
                @click="mcp.startServer(server.id)"
              >
                Restart
              </KxButton>
              <KxButton
                v-if="server.status === 'running' && !trustedSet.has(server.id)"
                size="xs"
                data-test="mcp-trust-btn"
                @click="mcp.trustServer(server.id)"
              >
                Trust
              </KxButton>
              <KxButton
                v-if="trustedSet.has(server.id)"
                size="xs"
                data-test="mcp-revoke-btn"
                @click="mcp.revokeTrust(server.id)"
              >
                Revoke
              </KxButton>
              <KxButton
                v-if="server.status === 'running'"
                size="xs"
                data-test="mcp-refresh-btn"
                @click="mcp.refreshTools(server.id)"
              >
                Refresh
              </KxButton>
            </div>

            <span
              v-if="server.status === 'running' && server.tool_count"
              class="text-muted mcp-server-meta"
              data-test="mcp-tool-count"
            >
              {{ server.tool_count }} tools
            </span>
          </div>
        </li>
      </ul>
    </div>
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
  z-index: var(--app-z-palette);
}

/* Card chrome */
.card {
  background: var(--app-card-color, #fff);
  border: 1px solid var(--app-border-color, #d7d7d7);
  border-radius: 6px;
}
.mcp-manager-card {
  box-shadow: 0 -2px 8px var(--app-shadow-color, rgba(0, 0, 0, 0.12));
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color, #d7d7d7);
}
.card-title {
  font-size: 13px;
  color: var(--app-text-color, #333);
}

/* Close button */
/* Empty state */
.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
}
.mcp-empty-wrap {
  padding: 16px 0;
}
.mcp-empty {
  font-size: 12px;
  margin: 0;
  text-align: center;
  color: var(--app-text-color-3, #999);
}

/* List */
.list {
  list-style: none;
  margin: 0;
  padding: 0;
}
.list-item {
  padding: 8px 12px;
  transition: background 0.15s;
}
.list-item:hover {
  background: var(--app-hover-color, #f8f8f8);
}

/* Server item layout */
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
  color: var(--app-text-color, #333);
}

/* Tags */
.tag {
  display: inline-block;
  padding: 0 6px;
  border-radius: 3px;
  line-height: 1.6;
}
.mcp-server-status,
.mcp-trusted,
.mcp-untrusted {
  font-size: 10px;
}
.tag-success {
  background: var(--app-success-bg, #e8f5e9);
  color: var(--app-success-color, #18a058);
}
.tag-warning {
  background: var(--app-warning-bg, #fff8e1);
  color: var(--app-warning-color, #b45309);
}
.tag-error {
  background: var(--app-error-bg, #fff5f5);
  color: var(--app-error-color, #d03050);
}
.tag-default {
  background: var(--app-hover-color, #f0f0f0);
  color: var(--app-text-color-3, #888);
}

/* Error text */
.text-error {
  color: var(--app-error-color, #d03050);
}
.mcp-server-error {
  font-size: 11px;
  overflow-wrap: anywhere;
}

/* Muted text */
.text-muted {
  color: var(--app-text-color-3, #999);
}
.mcp-server-meta {
  font-size: 11px;
}

/* Flex wrap for action buttons */
.flex-wrap {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
</style>

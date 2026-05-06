<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { NSpace, NTag, NText, NTooltip } from "naive-ui";
import { useSessionStore } from "@/stores/session";
import { useMcpStore } from "@/stores/mcp";
import McpStatusIndicator from "./McpStatusIndicator.vue";
import McpServerManager from "./McpServerManager.vue";

const session = useSessionStore();
const mcp = useMcpStore();
const permissionMode = ref("interactive");
const showMcpManager = ref(false);

const streamingType = computed<"warning" | "default">(() =>
  session.isStreaming ? "warning" : "default"
);
const connectedType = computed<"success" | "error">(() =>
  session.connected ? "success" : "error"
);

onMounted(async () => {
  try {
    const mode: string = await invoke("get_permission_mode");
    // Convert PascalCase to lowercase for display
    permissionMode.value = mode.toLowerCase();
  } catch {
    permissionMode.value = "interactive";
  }
  // Fetch MCP server status on mount
  try {
    await mcp.fetchServers();
  } catch {
    // Non-critical — status indicator will just show empty state
  }
});
</script>

<template>
  <footer class="status-bar" data-test="status-bar">
    <NSpace size="small" align="center" :wrap="false">
      <NTooltip trigger="hover">
        <template #trigger>
          <NTag size="small" :bordered="false" data-test="status-profile">
            profile: {{ session.currentProfile }}
          </NTag>
        </template>
        Active profile
      </NTooltip>

      <NText depth="3" class="status-item" data-test="status-sessions">
        sessions: {{ session.sessions.length }}
      </NText>

      <NTag
        size="small"
        :bordered="false"
        :type="streamingType"
        data-test="status-streaming"
      >
        {{ session.isStreaming ? "streaming: yes" : "streaming: no" }}
      </NTag>

      <NTag
        size="small"
        :bordered="false"
        :type="connectedType"
        data-test="status-connected"
      >
        {{ session.connected ? "connected: yes" : "connected: no" }}
      </NTag>

      <NText depth="3" class="status-item" data-test="status-mode">
        mode: {{ permissionMode }}
      </NText>

      <span class="mcp-status-wrapper">
        <McpStatusIndicator @click="showMcpManager = !showMcpManager" />
        <McpServerManager
          v-if="showMcpManager"
          @close="showMcpManager = false"
        />
      </span>
    </NSpace>
  </footer>
</template>

<style scoped>
.status-bar {
  padding: 4px 16px;
  background: var(--app-card-color, #f5f5f5);
  border-top: 1px solid var(--app-border-color, #d7d7d7);
  font-size: 11px;
  color: var(--app-text-color, #555);
}
.status-item {
  font-size: 11px;
}
.mcp-status-wrapper {
  position: relative;
  display: inline-flex;
  align-items: center;
}
</style>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { sessionState } from "../stores/session";
import { fetchServers } from "../stores/mcp";
import McpStatusIndicator from "./McpStatusIndicator.vue";
import McpServerManager from "./McpServerManager.vue";

const permissionMode = ref("interactive");
const showMcpManager = ref(false);

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
    await fetchServers();
  } catch {
    // Non-critical — status indicator will just show empty state
  }
});
</script>

<template>
  <footer class="status-bar">
    <span class="status-item">profile: {{ sessionState.currentProfile }}</span>
    <span class="status-divider">│</span>
    <span class="status-item"
      >sessions: {{ sessionState.sessions.length }}</span
    >
    <span class="status-divider">│</span>
    <span class="status-item">{{
      sessionState.isStreaming ? "streaming: yes" : "streaming: no"
    }}</span>
    <span class="status-divider">│</span>
    <span class="status-item">{{
      sessionState.connected ? "connected: yes" : "connected: no"
    }}</span>
    <span class="status-divider">│</span>
    <span class="status-item">mode: {{ permissionMode }}</span>
    <span class="status-divider">│</span>
    <span class="status-item mcp-status-wrapper">
      <McpStatusIndicator @click="showMcpManager = !showMcpManager" />
      <McpServerManager v-if="showMcpManager" @close="showMcpManager = false" />
    </span>
  </footer>
</template>

<style scoped>
.status-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 16px;
  background: #f5f5f5;
  border-top: 1px solid #d7d7d7;
  font-size: 11px;
  color: #555;
}
.status-divider {
  color: #ccc;
}
.mcp-status-wrapper {
  position: relative;
}
</style>

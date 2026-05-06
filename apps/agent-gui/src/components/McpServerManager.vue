<script setup lang="ts">
import { useMcpStore } from "@/stores/mcp";

const mcp = useMcpStore();
const emit = defineEmits<{ close: [] }>();

function statusLabel(status: string): string {
  switch (status) {
    case "running":
      return "🟢 Running";
    case "starting":
      return "🟡 Starting";
    case "failed":
      return "🔴 Failed";
    default:
      return "⚪ Stopped";
  }
}
</script>

<template>
  <div class="mcp-manager">
    <div class="mcp-manager-header">
      <h3>MCP Servers</h3>
      <button class="mcp-close-btn" @click="emit('close')">✕</button>
    </div>
    <div class="mcp-manager-list">
      <div
        v-for="server in mcp.servers"
        :key="server.id"
        class="mcp-server-item"
      >
        <div class="mcp-server-info">
          <span class="mcp-server-name">{{ server.id }}</span>
          <span class="mcp-server-status">{{
            statusLabel(server.status)
          }}</span>
          <span
            v-if="mcp.trustedServerIds.includes(server.id)"
            class="mcp-trusted"
            >✅ Trusted</span
          >
          <span v-else-if="server.status === 'running'" class="mcp-untrusted"
            >⚠️ Not trusted</span
          >
        </div>
        <div
          v-if="server.status === 'failed' && server.error"
          class="mcp-server-error"
        >
          {{ server.error }}
        </div>
        <div class="mcp-server-actions">
          <button
            v-if="server.status === 'stopped'"
            @click="mcp.startServer(server.id)"
          >
            Start
          </button>
          <button
            v-if="server.status === 'running'"
            @click="mcp.stopServer(server.id)"
          >
            Stop
          </button>
          <button
            v-if="server.status === 'failed'"
            @click="mcp.startServer(server.id)"
          >
            Restart
          </button>
          <button
            v-if="
              server.status === 'running' &&
              !mcp.trustedServerIds.includes(server.id)
            "
            @click="mcp.trustServer(server.id)"
          >
            Trust
          </button>
          <button
            v-if="mcp.trustedServerIds.includes(server.id)"
            @click="mcp.revokeTrust(server.id)"
          >
            Revoke
          </button>
          <button
            v-if="server.status === 'running'"
            @click="mcp.refreshTools(server.id)"
          >
            Refresh
          </button>
        </div>
        <div
          v-if="server.status === 'running' && server.tool_count"
          class="mcp-server-meta"
        >
          {{ server.tool_count }} tools
        </div>
      </div>
      <p v-if="mcp.servers.length === 0" class="mcp-empty">
        No MCP servers configured
      </p>
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
  background: #fff;
  border: 1px solid #d7d7d7;
  border-radius: 6px;
  box-shadow: 0 -2px 8px rgba(0, 0, 0, 0.1);
  z-index: 100;
}
.mcp-manager-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid #e0e0e0;
}
.mcp-manager-header h3 {
  margin: 0;
  font-size: 13px;
}
.mcp-close-btn {
  background: none;
  border: none;
  font-size: 16px;
  cursor: pointer;
  padding: 0 4px;
  line-height: 1;
}
.mcp-close-btn:hover {
  opacity: 0.7;
}
.mcp-manager-list {
  padding: 8px 12px;
}
.mcp-server-item {
  padding: 8px 0;
  border-bottom: 1px solid #f0f0f0;
}
.mcp-server-item:last-child {
  border-bottom: none;
}
.mcp-server-info {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.mcp-server-name {
  font-weight: 600;
  font-size: 12px;
}
.mcp-server-status {
  font-size: 11px;
}
.mcp-trusted {
  font-size: 11px;
  color: #22a06b;
}
.mcp-untrusted {
  font-size: 11px;
  color: #e6a700;
}
.mcp-server-error {
  font-size: 11px;
  color: #d93025;
  margin-top: 4px;
  overflow-wrap: anywhere;
}
.mcp-server-actions {
  display: flex;
  gap: 4px;
  margin-top: 6px;
}
.mcp-server-actions button {
  padding: 2px 8px;
  font-size: 11px;
  border: 1px solid #ccc;
  border-radius: 3px;
  background: #f5f5f5;
  cursor: pointer;
}
.mcp-server-actions button:hover {
  background: #e8e8e8;
}
.mcp-server-meta {
  font-size: 11px;
  color: #777;
  margin-top: 4px;
}
.mcp-empty {
  font-size: 12px;
  color: #999;
  text-align: center;
  margin: 16px 0;
}
</style>

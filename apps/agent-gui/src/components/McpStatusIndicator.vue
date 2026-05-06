<script setup lang="ts">
import { computed } from "vue";
import { useMcpStore } from "@/stores/mcp";

const mcp = useMcpStore();
const emit = defineEmits<{ click: [] }>();

const indicatorClass = computed(() => {
  if (mcp.failedServers.length > 0) return "mcp-failed";
  if (mcp.runningCount > 0) return "mcp-running";
  if (mcp.hasServers) return "mcp-stopped";
  return "mcp-none";
});

const label = computed(() => {
  if (!mcp.hasServers) return "MCP";
  return `${mcp.runningCount} MCP`;
});

const dot = computed(() => {
  if (mcp.failedServers.length > 0) return "🔴";
  if (mcp.runningCount > 0) return "🟢";
  return "⚪";
});
</script>

<template>
  <span class="mcp-status" :class="indicatorClass" @click="emit('click')">
    {{ dot }} {{ label }}
  </span>
</template>

<style scoped>
.mcp-status {
  cursor: pointer;
  font-size: 0.8em;
  padding: 0 6px;
  user-select: none;
}
.mcp-status:hover {
  opacity: 0.8;
}
</style>

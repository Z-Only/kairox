<script setup lang="ts">
import { computed } from "vue";
import { failedServers, runningCount, hasServers } from "../stores/mcp";

const emit = defineEmits<{ click: [] }>();

const indicatorClass = computed(() => {
  if (failedServers.value.length > 0) return "mcp-failed";
  if (runningCount.value > 0) return "mcp-running";
  if (hasServers.value) return "mcp-stopped";
  return "mcp-none";
});

const label = computed(() => {
  if (!hasServers.value) return "MCP";
  return `${runningCount.value} MCP`;
});

const dot = computed(() => {
  if (failedServers.value.length > 0) return "🔴";
  if (runningCount.value > 0) return "🟢";
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

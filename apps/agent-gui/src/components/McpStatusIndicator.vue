<script setup lang="ts">
import { computed } from "vue";
import { NText } from "naive-ui";
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

// Map indicator state → NText `type` so coloring follows the active
// NaiveUI theme (light/dark) instead of the hard-coded colours we
// previously baked into the dot emojis.
const textType = computed<"default" | "success" | "warning" | "error">(() => {
  if (mcp.failedServers.length > 0) return "error";
  if (mcp.runningCount > 0) return "success";
  if (mcp.hasServers) return "warning";
  return "default";
});
</script>

<template>
  <!-- Outer span + .mcp-status / .mcp-failed|running|stopped|none class
       hooks are preserved verbatim so the existing test suite (which
       asserts on these classes) keeps passing after the NaiveUI move.
       NText handles theme-aware colouring of the label. -->
  <span class="mcp-status" :class="indicatorClass" @click="emit('click')">
    <NText :type="textType">{{ dot }} {{ label }}</NText>
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

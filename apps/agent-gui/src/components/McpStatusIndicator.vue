<script setup lang="ts">
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

const textColorVar = computed(() => {
  if (mcp.failedServers.length > 0) return "var(--app-error-color)";
  if (mcp.runningCount > 0) return "var(--app-success-color)";
  if (mcp.hasServers) return "var(--app-warning-color)";
  return "var(--app-text-color-2)";
});
</script>

<template>
  <!-- Outer span + .mcp-status / .mcp-failed|running|stopped|none class
       hooks are preserved verbatim so the existing test suite (which
       asserts on these classes) keeps passing.
       Theme-aware colouring of the label is handled via CSS variables. -->
  <span
    class="mcp-status"
    data-test="mcp-status-indicator"
    :class="indicatorClass"
    :style="{ color: textColorVar }"
    role="button"
    tabindex="0"
    @click="emit('click')"
    @keydown.enter="emit('click')"
    @keydown.space.prevent="emit('click')"
  >
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

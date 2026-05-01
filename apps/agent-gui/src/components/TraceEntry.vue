<script setup lang="ts">
import type { TraceEntryData } from "../types/trace";
import { traceState } from "../composables/useTraceStore";
import PermissionPrompt from "./PermissionPrompt.vue";

const props = defineProps<{
  entry: TraceEntryData;
  density: "L1" | "L2" | "L3";
}>();

function toggle() {
  const found = traceState.entries.find((e) => e.id === props.entry.id);
  if (found) {
    found.expanded = !found.expanded;
  }
}

const statusIcon: Record<string, string> = {
  running: "⏳",
  completed: "✅",
  failed: "❌",
  pending: "🔑"
};
</script>

<template>
  <div
    :class="[
      'trace-entry',
      `trace-entry--${entry.status}`,
      `trace-entry--${entry.kind}`
    ]"
  >
    <PermissionPrompt
      v-if="entry.kind === 'permission' && entry.status === 'pending'"
      :entry="entry"
    />
    <template v-else>
      <div class="entry-row" @click="toggle">
        <span class="entry-status">{{ statusIcon[entry.status] }}</span>
        <span class="entry-tool">{{ entry.toolId || entry.title }}</span>
        <span v-if="entry.durationMs != null" class="entry-duration">
          {{ (entry.durationMs / 1000).toFixed(1) }}s
        </span>
        <span v-if="entry.status === 'running'" class="entry-running"
          >running...</span
        >
      </div>
      <div v-if="density !== 'L1' && entry.expanded" class="entry-detail">
        <div v-if="entry.input" class="entry-section">
          <span class="entry-label">Input:</span>
          <pre class="entry-code">{{ entry.input }}</pre>
        </div>
        <div v-if="entry.outputPreview" class="entry-section">
          <span class="entry-label">Output:</span>
          <pre class="entry-code">{{ entry.outputPreview }}</pre>
        </div>
        <div v-if="entry.reason" class="entry-section">
          <span class="entry-label">Reason:</span>
          <span>{{ entry.reason }}</span>
        </div>
        <div
          v-if="entry.content && entry.kind === 'memory'"
          class="entry-section"
        >
          <span class="entry-label">Content:</span>
          <pre class="entry-code">{{ entry.content }}</pre>
        </div>
      </div>
      <div
        v-if="density === 'L3' && entry.expanded && entry.rawEvent"
        class="entry-raw"
      >
        <pre class="entry-code">{{ entry.rawEvent }}</pre>
      </div>
    </template>
  </div>
</template>

<style scoped>
.trace-entry {
  font-size: 12px;
  border-bottom: 1px solid #eee;
}
.entry-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
  cursor: pointer;
}
.entry-row:hover {
  background: #f8f8f8;
}
.entry-status {
  font-size: 11px;
}
.entry-tool {
  flex: 1;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.entry-duration {
  color: #777;
  font-size: 11px;
}
.entry-running {
  color: #0077cc;
  font-size: 11px;
}
.entry-detail,
.entry-raw {
  padding: 4px 8px 8px;
  background: #fafafa;
}
.entry-section {
  margin-bottom: 4px;
}
.entry-label {
  font-weight: 600;
  font-size: 11px;
  color: #555;
}
.entry-code {
  margin: 2px 0 0;
  padding: 6px 8px;
  background: #1e1e2e;
  color: #cdd6f4;
  border-radius: 4px;
  font-size: 11px;
  line-height: 1.4;
  overflow-x: auto;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>

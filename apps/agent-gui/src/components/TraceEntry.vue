<script setup lang="ts">
import type { TraceEntryData } from "../types/trace";
import { traceState } from "../composables/useTraceStore";

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

const kindIcon: Record<string, string> = {
  tool: "🔧",
  permission: "🔑",
  memory: "🧠"
};

const hasTaskTitleDetail = computed(
  () =>
    props.entry.toolId === "task" &&
    Boolean(props.entry.title) &&
    !props.entry.input &&
    !props.entry.outputPreview &&
    !props.entry.reason &&
    !(props.entry.content && props.entry.kind === "memory")
);

const hasContextTitleDetail = computed(
  () =>
    props.entry.toolId === "context" &&
    Boolean(props.entry.title) &&
    !props.entry.input &&
    !props.entry.outputPreview &&
    !props.entry.reason
);

const hasDetail = computed(
  () =>
    Boolean(
      props.entry.input ||
      props.entry.outputPreview ||
      props.entry.reason ||
      (props.entry.content && props.entry.kind === "memory")
    ) ||
    hasTaskTitleDetail.value ||
    hasContextTitleDetail.value
);
</script>

<template>
  <div
    :class="['trace-entry', `trace-entry--${entry.status}`, `trace-entry--${entry.kind}`]"
    data-test="trace-entry"
  >
    <!-- All entries show as a trace row; pending permission/memory
         interactions are handled exclusively by inline chat-stream
         items (ChatPermissionItem) rendered inside ChatPanel. The
         row's bespoke class names are preserved because the unit tests
         (TraceEntry.test.ts) assert against `.entry-row`, `.entry-status`,
         `.entry-detail`, and `.entry-duration` — using CSS-class-based
         primitives inside the row keeps those selectors stable. -->
    <div class="entry-row" @click="toggle">
      <span class="entry-icon">{{ kindIcon[entry.kind] || "•" }}</span>
      <span class="entry-status">{{ statusIcon[entry.status] }}</span>
      <span class="entry-tool">
        <span class="truncate">
          {{ entry.toolId || entry.title }}
        </span>
      </span>
      <KxTag v-if="entry.scope" class="entry-scope" tone="info" size="sm">
        {{ entry.scope }}
      </KxTag>
      <span
        v-if="entry.durationMs != null"
        :style="{ color: 'var(--app-text-color-3)' }"
        class="entry-duration"
      >
        {{ (entry.durationMs / 1000).toFixed(1) }}s
      </span>
      <span
        v-if="entry.status === 'running'"
        :style="{ color: 'var(--app-info-color)' }"
        class="entry-running"
      >
        running...
      </span>
      <KxBadge v-if="entry.status === 'pending'" class="entry-pending" tone="warning">
        pending
      </KxBadge>
    </div>
    <div v-if="entry.expanded && hasDetail" class="entry-detail">
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
      <div v-if="entry.content && entry.kind === 'memory'" class="entry-section">
        <span class="entry-label">Content:</span>
        <pre class="entry-code">{{ entry.content }}</pre>
      </div>
      <div v-if="hasTaskTitleDetail" class="entry-section">
        <span class="entry-label">Task:</span>
        <pre class="entry-code">{{ entry.title }}</pre>
      </div>
      <div v-if="hasContextTitleDetail" class="entry-section">
        <span class="entry-label">Context:</span>
        <pre class="entry-code">{{ entry.title }}</pre>
      </div>
    </div>
    <div v-if="density === 'L3' && entry.expanded && entry.rawEvent" class="entry-raw">
      <pre class="entry-code">{{ entry.rawEvent }}</pre>
    </div>
  </div>
</template>

<style scoped>
.trace-entry {
  font-size: 12px;
  box-sizing: border-box;
  min-width: 0;
  max-width: 100%;
  border-bottom: 1px solid var(--app-border-color);
  overflow-x: hidden;
}
.trace-entry--pending {
  background: color-mix(in srgb, var(--app-warning-color) 8%, transparent);
}
.entry-row {
  display: flex;
  min-width: 0;
  max-width: 100%;
  align-items: center;
  gap: 4px;
  padding: 5px 8px;
  cursor: pointer;
}
.entry-row:hover {
  background: var(--app-hover-color);
}
.entry-icon {
  font-size: 11px;
}
.entry-status {
  font-size: 11px;
}
.entry-tool {
  flex: 1;
  min-width: 0;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.entry-scope {
  font-size: 10px;
}
.entry-duration {
  color: var(--app-text-color-3);
  font-size: 11px;
}
.entry-running {
  color: var(--app-info-color);
  font-size: 11px;
}
.entry-pending {
  font-size: 10px;
}
.entry-detail,
.entry-raw {
  padding: 4px 8px 8px;
  background: var(--app-card-color);
}
.entry-section {
  margin-bottom: 4px;
}
.entry-label {
  font-weight: 600;
  font-size: 11px;
  color: var(--app-text-color-2);
}
.entry-code {
  margin: 2px 0 0;
  padding: 6px 8px;
  background: var(--app-code-bg);
  color: var(--app-text-color);
  border-radius: 4px;
  font-size: 11px;
  line-height: 1.4;
  overflow-x: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>

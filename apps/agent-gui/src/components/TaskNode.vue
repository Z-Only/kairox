<script setup lang="ts">
import type { TaskTreeNode } from "@/stores/taskGraph";
import { useTaskGraphStore } from "@/stores/taskGraph";

const taskGraph = useTaskGraphStore();

const props = defineProps<{
  node: TaskTreeNode;
  expanded: Set<string>;
  depth: number;
}>();

const emit = defineEmits<{
  (e: "toggle-expand", taskId: string): void;
}>();

const statusIcon: Record<string, string> = {
  Pending: "⏳",
  Ready: "⏳",
  Running: "🔄",
  Blocked: "⏸️",
  Completed: "✅",
  Failed: "❌",
  Skipped: "⏭️",
  Cancelled: "🚫"
};

const roleLabel: Record<string, string> = {
  Planner: "P",
  Worker: "W",
  Reviewer: "R"
};

// Inline RGB values are asserted by `TaskNode.test.ts` (jsdom converts hex
// `#RRGGBB` styles to `rgb(r, g, b)` form). Keep the color literals here so
// future refactors do not silently change the asserted bytes.
const roleColor: Record<string, string> = {
  Planner: "#0077cc",
  Worker: "#22a06b",
  Reviewer: "#7c3aed"
};

const isExpanded = computed(() => props.expanded.has(props.node.task.id));
const hasChildren = computed(() => props.node.children.length > 0);
const isFailed = computed(() => props.node.task.state === "Failed");
const isBlocked = computed(() => props.node.task.state === "Blocked");

const agentBadge = computed(() => {
  if (props.node.agentLabel) return props.node.agentLabel;
  return roleLabel[props.node.task.role] || "?";
});

const badgeColor = computed(() => {
  return roleColor[props.node.task.role] || "#666";
});

function retryLabel(): string {
  if (props.node.task.retry_count === 0) return "";
  return `↻${props.node.task.retry_count}/${props.node.task.max_retries}`;
}

function childSummary(): string {
  const counts: Record<string, number> = {};
  for (const c of props.node.children) {
    const icon = statusIcon[c.task.state] || "•";
    counts[icon] = (counts[icon] || 0) + 1;
  }
  return Object.entries(counts)
    .map(([icon, n]) => `${icon} ${n}`)
    .join(" · ");
}

function handleRetry() {
  taskGraph.retryTask(props.node.task.id);
}

function handleCancel() {
  taskGraph.cancelTask(props.node.task.id);
}

function handleToggle() {
  if (hasChildren.value) {
    emit("toggle-expand", props.node.task.id);
  }
}
</script>

<template>
  <div class="task-node-wrapper">
    <div
      :class="[
        'card',
        'task-node',
        `task-state-${node.task.state.toLowerCase()}`,
        {
          'task-root': depth === 0,
          'task-interactive': hasChildren || isFailed || isBlocked
        }
      ]"
      data-test="task-node"
      @click="handleToggle"
    >
      <div class="task-row">
        <span v-if="hasChildren" class="task-expand">
          {{ isExpanded ? "▾" : "▸" }}
        </span>
        <span v-else :style="{ paddingLeft: depth > 0 ? '0' : '12px' }" class="task-expand"> </span>
        <span v-if="depth > 0" class="task-indent"> {{ "│ ".repeat(depth - 1) }}├─ </span>
        <span class="task-status" data-test="task-node-status">
          {{ statusIcon[node.task.state] || "•" }}
        </span>
        <!-- Inline `background-color` is preserved instead of using NTag's
             `color` prop because the test asserts the inline `style`
             attribute literal directly. -->
        <span class="task-role" :style="{ backgroundColor: badgeColor }">
          {{ agentBadge }}
        </span>
        <span class="task-title">{{ node.task.title }}</span>
        <span v-if="retryLabel()" class="tag tag-warning task-retry">
          {{ retryLabel() }}
        </span>
        <span v-if="hasChildren && !isExpanded" class="task-summary">
          {{ childSummary() }}
        </span>
        <span v-if="node.task.state === 'Running'" class="task-running"> running... </span>
        <div v-if="isFailed" class="task-actions">
          <button class="btn btn-icon btn-retry" title="Retry task" @click.stop="handleRetry">
            ↻
          </button>
          <button class="btn btn-icon btn-cancel" title="Cancel task" @click.stop="handleCancel">
            ✕
          </button>
        </div>
        <div v-if="isBlocked" class="task-actions">
          <button
            class="btn btn-icon btn-cancel"
            title="Cancel blocked task"
            @click.stop="handleCancel"
          >
            ✕
          </button>
        </div>
      </div>
    </div>
    <div v-if="node.task.error" class="task-error" :style="{ paddingLeft: `${depth * 16 + 8}px` }">
      <span class="task-error-text">
        {{ node.task.error }}
      </span>
    </div>
    <hr v-if="depth === 0 && isExpanded && hasChildren" class="divider task-divider" />
    <div v-if="isExpanded && hasChildren" class="task-children">
      <TaskNode
        v-for="child in node.children"
        :key="child.task.id"
        :node="child"
        :expanded="expanded"
        :depth="depth + 1"
        @toggle-expand="(id: string) => emit('toggle-expand', id)"
      />
    </div>
  </div>
</template>

<style scoped>
.task-node-wrapper {
  /* Container for node + children */
}
.task-node {
  font-size: 12px;
  cursor: default;
  padding: 4px 8px;
}
.task-interactive {
  cursor: pointer;
}
.task-interactive:hover {
  background: var(--app-hover-color, #f0f4f8);
}
.task-row {
  display: flex;
  gap: 4px;
  align-items: center;
  flex-wrap: nowrap;
  min-height: 20px;
  width: 100%;
}
.task-expand {
  width: 12px;
  font-size: 10px;
  color: var(--app-text-color-3, #777);
  flex-shrink: 0;
}
.task-indent {
  color: var(--app-divider-color, #ccc);
  font-size: 11px;
  flex-shrink: 0;
  white-space: pre;
  font-family: monospace;
}
.task-status {
  font-size: 11px;
  flex-shrink: 0;
}
.task-role {
  font-size: 10px;
  font-weight: 600;
  color: white;
  border-radius: 3px;
  padding: 0 4px;
  line-height: 16px;
  flex-shrink: 0;
}
.task-title {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
}
.task-retry {
  font-size: 10px;
  flex-shrink: 0;
}
.task-summary {
  font-size: 10px;
  flex-shrink: 0;
  color: var(--app-text-color-3, #999);
}
.task-running {
  font-size: 10px;
  flex-shrink: 0;
  color: var(--app-info-color, #2080f0);
}
.btn-icon {
  background: none;
  border: none;
  cursor: pointer;
  padding: 2px;
  font-size: 12px;
  line-height: 1;
  border-radius: 50%;
  color: var(--app-text-color-2, #555);
}
.btn-icon:hover {
  background: var(--app-hover-color, #f0f4f8);
}
.task-actions {
  display: flex;
  gap: 2px;
  align-items: center;
  flex-wrap: nowrap;
  flex-shrink: 0;
  margin-left: 4px;
}
.task-error {
  display: flex;
  padding: 2px 8px;
}
.task-error-text {
  font-size: 11px;
  background: var(--app-error-bg, #fff5f5);
  color: var(--app-error-color, #cc3333);
  border-radius: 3px;
  padding: 2px 6px;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.task-divider {
  margin: 4px 0;
}
.task-state-failed .task-title {
  color: var(--app-error-color, #cc3333);
}
.task-state-blocked .task-title {
  color: var(--app-warning-color, #b45309);
}
.task-state-skipped .task-title {
  color: var(--app-text-color-3, #888);
}
.task-children {
  /* N-level nesting via recursive TaskNode */
}
</style>

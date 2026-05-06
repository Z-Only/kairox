<script setup lang="ts">
import { computed } from "vue";
import type { TaskTreeNode } from "../stores/taskGraph";
import { retryTask, cancelTask } from "../stores/taskGraph";

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
  retryTask(props.node.task.id);
}

function handleCancel() {
  cancelTask(props.node.task.id);
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
        'task-node',
        `task-state-${node.task.state.toLowerCase()}`,
        {
          'task-root': depth === 0,
          'task-interactive': hasChildren || isFailed || isBlocked
        }
      ]"
      @click="handleToggle"
    >
      <span v-if="hasChildren" class="task-expand">
        {{ isExpanded ? "▾" : "▸" }}
      </span>
      <span
        v-else
        :style="{ paddingLeft: depth > 0 ? '0' : '12px' }"
        class="task-expand"
      >
      </span>
      <span v-if="depth > 0" class="task-indent">
        {{ "│ ".repeat(depth - 1) }}├─
      </span>
      <span class="task-status">{{ statusIcon[node.task.state] || "•" }}</span>
      <span class="task-role" :style="{ backgroundColor: badgeColor }">
        {{ agentBadge }}
      </span>
      <span class="task-title">{{ node.task.title }}</span>
      <span v-if="retryLabel()" class="task-retry">{{ retryLabel() }}</span>
      <span v-if="hasChildren && !isExpanded" class="task-summary">
        {{ childSummary() }}
      </span>
      <span v-if="node.task.state === 'Running'" class="task-running">
        running...
      </span>
      <span v-if="isFailed" class="task-actions">
        <button title="Retry task" class="btn-retry" @click.stop="handleRetry">
          ↻
        </button>
        <button
          title="Cancel task"
          class="btn-cancel"
          @click.stop="handleCancel"
        >
          ✕
        </button>
      </span>
      <span v-if="isBlocked" class="task-actions">
        <button
          title="Cancel blocked task"
          class="btn-cancel"
          @click.stop="handleCancel"
        >
          ✕
        </button>
      </span>
    </div>
    <div
      v-if="node.task.error"
      class="task-error"
      :style="{ paddingLeft: `${depth * 16 + 8}px` }"
    >
      <span class="task-error-text">{{ node.task.error }}</span>
    </div>
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
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  font-size: 12px;
  cursor: default;
  min-height: 28px;
}
.task-interactive {
  cursor: pointer;
}
.task-interactive:hover {
  background: #f0f4f8;
}
.task-expand {
  width: 12px;
  font-size: 10px;
  color: #777;
  flex-shrink: 0;
}
.task-indent {
  color: #ccc;
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
  color: #b45309;
  flex-shrink: 0;
}
.task-summary {
  font-size: 10px;
  color: #777;
  flex-shrink: 0;
}
.task-running {
  font-size: 10px;
  color: #0077cc;
  flex-shrink: 0;
}
.task-actions {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
  margin-left: 4px;
}
.btn-retry,
.btn-cancel {
  border: none;
  background: none;
  cursor: pointer;
  font-size: 12px;
  padding: 0 4px;
  border-radius: 3px;
  line-height: 16px;
}
.btn-retry {
  color: #0077cc;
}
.btn-retry:hover {
  background: #e0f0ff;
}
.btn-cancel {
  color: #999;
}
.btn-cancel:hover {
  color: #cc3333;
  background: #fff0f0;
}
.task-error {
  display: flex;
  padding: 2px 8px;
}
.task-error-text {
  font-size: 11px;
  color: #cc3333;
  background: #fff5f5;
  border-radius: 3px;
  padding: 2px 6px;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.task-state-failed .task-title {
  color: #cc3333;
}
.task-state-blocked .task-title {
  color: #b45309;
}
.task-state-skipped .task-title {
  color: #888;
}
.task-children {
  /* N-level nesting via recursive TaskNode */
}
</style>

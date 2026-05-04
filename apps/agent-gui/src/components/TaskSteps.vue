<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  taskGraphState,
  buildTaskTree,
  type TaskTreeNode
} from "../stores/taskGraph";

const tree = computed(() => buildTaskTree(taskGraphState.tasks));

const statusIcon: Record<string, string> = {
  Pending: "⏳",
  Running: "🔄",
  Blocked: "⏸️",
  Completed: "✅",
  Failed: "❌",
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

const expanded = ref<Set<string>>(new Set());

// Auto-expand root nodes when tree changes
watch(
  () => tree.value,
  (newTree) => {
    const newExpanded = new Set(expanded.value);
    for (const root of newTree) {
      if (!newExpanded.has(root.task.id) && root.children.length > 0) {
        newExpanded.add(root.task.id);
      }
    }
    expanded.value = newExpanded;
  },
  { immediate: true }
);

function toggleExpand(taskId: string) {
  const next = new Set(expanded.value);
  if (next.has(taskId)) {
    next.delete(taskId);
  } else {
    next.add(taskId);
  }
  expanded.value = next;
}

function childSummary(children: TaskTreeNode[]): string {
  const counts: Record<string, number> = {};
  for (const c of children) {
    const icon = statusIcon[c.task.state] || "•";
    counts[icon] = (counts[icon] || 0) + 1;
  }
  return Object.entries(counts)
    .map(([icon, n]) => `${icon} ${n}`)
    .join(" · ");
}
</script>

<template>
  <div class="task-steps">
    <div v-if="tree.length === 0" class="empty-hint">No tasks yet</div>
    <template v-for="root in tree" :key="root.task.id">
      <div
        :class="[
          'task-node',
          'task-root',
          `task-state-${root.task.state.toLowerCase()}`
        ]"
        @click="toggleExpand(root.task.id)"
      >
        <span v-if="root.children.length > 0" class="task-expand">
          {{ expanded.has(root.task.id) ? "▾" : "▸" }}
        </span>
        <span v-else class="task-expand"> </span>
        <span class="task-status">{{
          statusIcon[root.task.state] || "•"
        }}</span>
        <span
          class="task-role"
          :style="{ backgroundColor: roleColor[root.task.role] || '#666' }"
        >
          {{ roleLabel[root.task.role] || "?" }}
        </span>
        <span class="task-title">{{ root.task.title }}</span>
        <span
          v-if="root.children.length > 0 && !expanded.has(root.task.id)"
          class="task-summary"
        >
          {{ childSummary(root.children) }}
        </span>
        <span v-if="root.task.state === 'Running'" class="task-running">
          running...
        </span>
      </div>
      <div v-if="expanded.has(root.task.id)" class="task-children">
        <template v-for="child in root.children" :key="child.task.id">
          <div
            :class="[
              'task-node',
              `task-state-${child.task.state.toLowerCase()}`
            ]"
          >
            <span class="task-indent">├─</span>
            <span class="task-status">{{
              statusIcon[child.task.state] || "•"
            }}</span>
            <span
              class="task-role"
              :style="{ backgroundColor: roleColor[child.task.role] || '#666' }"
            >
              {{ roleLabel[child.task.role] || "?" }}
            </span>
            <span class="task-title">{{ child.task.title }}</span>
            <span v-if="child.task.state === 'Running'" class="task-running">
              running...
            </span>
          </div>
          <div v-if="child.task.error" class="task-error">
            <span class="task-indent">│ </span>
            <span class="task-error-text">{{ child.task.error }}</span>
          </div>
        </template>
      </div>
    </template>
  </div>
</template>

<style scoped>
.task-steps {
  padding: 4px 0;
  overflow-y: auto;
  flex: 1;
}
.empty-hint {
  padding: 12px;
  color: #999;
  font-size: 12px;
}
.task-node {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  font-size: 12px;
  cursor: default;
}
.task-root {
  cursor: pointer;
}
.task-root:hover {
  background: #f0f4f8;
}
.task-expand {
  width: 12px;
  font-size: 10px;
  color: #777;
  flex-shrink: 0;
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
.task-children {
  padding-left: 8px;
}
.task-indent {
  color: #ccc;
  font-size: 11px;
  flex-shrink: 0;
  width: 16px;
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
</style>

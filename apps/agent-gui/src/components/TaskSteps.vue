<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { taskGraphState, buildTaskTree } from "../stores/taskGraph";
import TaskNode from "./TaskNode.vue";

const tree = computed(() => buildTaskTree(taskGraphState.tasks));

/** Track expanded state per task ID. */
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
</script>

<template>
  <div class="task-steps">
    <div v-if="tree.length === 0" class="empty-hint">No tasks yet</div>
    <TaskNode
      v-for="root in tree"
      :key="root.task.id"
      :node="root"
      :expanded="expanded"
      :depth="0"
      @toggle-expand="toggleExpand"
    />
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
</style>

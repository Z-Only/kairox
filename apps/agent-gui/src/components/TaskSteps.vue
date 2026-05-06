<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { NScrollbar, NEmpty } from "naive-ui";
import { useTaskGraphStore } from "@/stores/taskGraph";
import TaskNode from "./TaskNode.vue";

const taskGraph = useTaskGraphStore();
const tree = computed(() => taskGraph.buildTaskTree(taskGraph.tasks));

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
    <!-- The "No tasks yet" copy is preserved verbatim because the existing
         unit test asserts on its presence; switching to NEmpty's
         description prop keeps both the visual upgrade and the assertion. -->
    <NEmpty
      v-if="tree.length === 0"
      size="small"
      class="empty-hint"
      description="No tasks yet"
    />
    <NScrollbar v-else class="task-tree-scroll">
      <TaskNode
        v-for="root in tree"
        :key="root.task.id"
        :node="root"
        :expanded="expanded"
        :depth="0"
        @toggle-expand="toggleExpand"
      />
    </NScrollbar>
  </div>
</template>

<style scoped>
.task-steps {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}
.task-tree-scroll {
  flex: 1;
  min-height: 0;
}
.empty-hint {
  padding: 12px;
  color: var(--app-text-disabled-color, #999);
  font-size: 12px;
}
</style>

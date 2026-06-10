<script setup lang="ts">
import { useAgentsStore } from "@/stores/agents";
import { useTaskGraphStore } from "@/stores/taskGraph";
import TaskNode from "./TaskNode.vue";

type TaskStateFilter = "all" | "active" | "failed" | "done";

const agents = useAgentsStore();
const taskGraph = useTaskGraphStore();
const { t } = useI18n();
const selectedFilter = ref<TaskStateFilter>("all");
const searchQuery = ref("");

const activeStates = new Set(["Pending", "Ready", "Running", "Blocked"]);
const doneStates = new Set(["Completed", "Skipped", "Cancelled"]);

function taskMatchesFilter(task: (typeof taskGraph.tasks)[number], filter: TaskStateFilter) {
  switch (filter) {
    case "active":
      return activeStates.has(task.state);
    case "failed":
      return task.state === "Failed";
    case "done":
      return doneStates.has(task.state);
    default:
      return true;
  }
}

function normalizeSearchText(value: string | null | undefined): string {
  return (value ?? "").toLocaleLowerCase();
}

function taskMatchesSearch(task: (typeof taskGraph.tasks)[number], query: string) {
  const normalizedQuery = normalizeSearchText(query).trim();
  if (!normalizedQuery) return true;

  const assignedAgentLabel = task.assigned_agent_id
    ? agents.agentLabel(task.assigned_agent_id)
    : "";
  const searchableText = [
    task.id,
    task.title,
    task.state,
    task.role,
    task.assigned_agent_id,
    assignedAgentLabel,
    ...task.dependencies,
    task.error
  ]
    .map((value) => normalizeSearchText(value))
    .join(" ");

  return searchableText.includes(normalizedQuery);
}

const filterOptions = computed<{ id: TaskStateFilter; label: string; count: number }[]>(() => [
  { id: "all", label: t("tasks.filterAll"), count: taskGraph.tasks.length },
  {
    id: "active",
    label: t("tasks.filterActive"),
    count: taskGraph.tasks.filter((task) => taskMatchesFilter(task, "active")).length
  },
  {
    id: "failed",
    label: t("tasks.filterFailed"),
    count: taskGraph.tasks.filter((task) => taskMatchesFilter(task, "failed")).length
  },
  {
    id: "done",
    label: t("tasks.filterDone"),
    count: taskGraph.tasks.filter((task) => taskMatchesFilter(task, "done")).length
  }
]);

const filteredTasks = computed(() =>
  taskGraph.tasks
    .filter((task) => taskMatchesSearch(task, searchQuery.value))
    .filter((task) => taskMatchesFilter(task, selectedFilter.value))
);
const tree = computed(() => taskGraph.buildTaskTree(filteredTasks.value));

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
  <div class="task-steps" data-test="task-steps">
    <KxChipGroup
      v-if="taskGraph.tasks.length > 0"
      class="task-state-filters"
      :aria-label="t('tasks.stateFiltersAria')"
      data-test="task-state-filters"
    >
      <KxChipButton
        v-for="option in filterOptions"
        :key="option.id"
        size="compact"
        :selected="selectedFilter === option.id"
        :data-test="`task-filter-${option.id}`"
        @click="selectedFilter = option.id"
      >
        {{ option.label }} {{ option.count }}
      </KxChipButton>
      <template #actions>
        <KxInput
          v-model="searchQuery"
          type="search"
          size="compact"
          :aria-label="t('tasks.searchAria')"
          :placeholder="t('tasks.searchPlaceholder')"
          class="task-search-input"
          data-test="task-search-input"
        />
      </template>
    </KxChipGroup>
    <KxEmptyState v-if="taskGraph.tasks.length === 0" class="task-empty" compact>
      {{ t("tasks.empty") }}
    </KxEmptyState>
    <KxEmptyState v-else-if="tree.length === 0" class="task-empty" compact>
      {{ t("tasks.emptyFiltered") }}
    </KxEmptyState>
    <div v-else class="task-tree-scroll" :style="{ overflowY: 'auto' }">
      <TaskNode
        v-for="root in tree"
        :key="root.task.id"
        :node="root"
        :expanded="expanded"
        :depth="0"
        @toggle-expand="toggleExpand"
      />
    </div>
  </div>
</template>

<style scoped>
.task-steps {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  max-width: 100%;
  min-height: 0;
}
.task-state-filters {
  box-sizing: border-box;
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color, #eee);
}
.task-search-input {
  flex: 0 1 220px;
}
.task-tree-scroll {
  box-sizing: border-box;
  flex: 1;
  min-width: 0;
  max-width: 100%;
  min-height: 0;
  overflow-x: hidden;
}
.task-empty {
  box-sizing: border-box;
  width: calc(100% - 24px);
  margin: 12px;
  font-size: 12px;
}
</style>

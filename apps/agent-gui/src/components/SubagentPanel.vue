<script setup lang="ts">
import { useAgentsStore, type AgentInfo } from "@/stores/agents";
import { useTaskGraphStore } from "@/stores/taskGraph";
import type { TaskSnapshot, TaskState } from "@/types";

type SubagentFilter = "all" | "running" | "attention" | "done";

interface SubagentRow {
  agent: AgentInfo;
  label: string;
  task: TaskSnapshot | null;
}

const agents = useAgentsStore();
const taskGraph = useTaskGraphStore();
const { t } = useI18n();
const selectedFilter = ref<SubagentFilter>("all");

const terminalStates = new Set<TaskState>(["Completed", "Failed", "Skipped", "Cancelled"]);
const cancellableStates = new Set<TaskState>(["Pending", "Ready", "Running", "Blocked", "Failed"]);

const taskById = computed(() => new Map(taskGraph.tasks.map((task) => [task.id, task])));
const taskByAgentId = computed(() => {
  const map = new Map<string, TaskSnapshot>();
  for (const task of taskGraph.tasks) {
    if (task.assigned_agent_id) {
      map.set(task.assigned_agent_id, task);
    }
  }
  return map;
});

function taskForAgent(agent: AgentInfo): TaskSnapshot | null {
  if (agent.taskId) {
    const task = taskById.value.get(agent.taskId);
    if (task) return task;
  }
  return taskByAgentId.value.get(agent.id) ?? null;
}

const rows = computed<SubagentRow[]>(() =>
  [...agents.agents.values()]
    .sort((a, b) => a.startedAt - b.startedAt || a.id.localeCompare(b.id))
    .map((agent) => ({
      agent,
      label: agents.agentLabel(agent.id),
      task: taskForAgent(agent)
    }))
);

function needsAttention(row: SubagentRow): boolean {
  return (
    row.agent.status === "failed" || row.task?.state === "Failed" || row.task?.state === "Blocked"
  );
}

function isDone(row: SubagentRow): boolean {
  return (
    row.agent.status === "completed" ||
    row.agent.status === "idle" ||
    (row.task ? terminalStates.has(row.task.state) : false)
  );
}

function matchesFilter(row: SubagentRow, filter: SubagentFilter): boolean {
  switch (filter) {
    case "running":
      return row.agent.status === "running" || row.task?.state === "Running";
    case "attention":
      return needsAttention(row);
    case "done":
      return isDone(row);
    default:
      return true;
  }
}

const filterOptions = computed<{ id: SubagentFilter; label: string; count: number }[]>(() => [
  { id: "all", label: t("subagents.filterAll"), count: rows.value.length },
  {
    id: "running",
    label: t("subagents.filterRunning"),
    count: rows.value.filter((row) => matchesFilter(row, "running")).length
  },
  {
    id: "attention",
    label: t("subagents.filterAttention"),
    count: rows.value.filter((row) => matchesFilter(row, "attention")).length
  },
  {
    id: "done",
    label: t("subagents.filterDone"),
    count: rows.value.filter((row) => matchesFilter(row, "done")).length
  }
]);

const filteredRows = computed(() =>
  rows.value.filter((row) => matchesFilter(row, selectedFilter.value))
);

const runningCount = computed(
  () => rows.value.filter((row) => matchesFilter(row, "running")).length
);
const attentionCount = computed(
  () => rows.value.filter((row) => matchesFilter(row, "attention")).length
);

function roleClass(role: string): string {
  return `subagent-role--${role.toLowerCase()}`;
}

function canRetry(task: TaskSnapshot | null): boolean {
  if (!task) return false;
  if (task.state !== "Failed" && task.state !== "Blocked") return false;
  return task.retry_count < task.max_retries;
}

function canCancel(task: TaskSnapshot | null): boolean {
  return task ? cancellableStates.has(task.state) : false;
}

function retryTask(row: SubagentRow): void {
  if (row.task) {
    void taskGraph.retryTask(row.task.id);
  }
}

function cancelTask(row: SubagentRow): void {
  if (row.task) {
    void taskGraph.cancelTask(row.task.id);
  }
}
</script>

<template>
  <section class="subagent-panel" data-test="subagent-panel">
    <div class="subagent-summary" data-test="subagent-summary">
      {{
        t("subagents.summary", {
          total: rows.length,
          running: runningCount,
          attention: attentionCount
        })
      }}
    </div>

    <KxChipGroup
      v-if="rows.length > 0"
      class="subagent-filters"
      :aria-label="t('subagents.filtersAria')"
      data-test="subagent-filters"
    >
      <KxChipButton
        v-for="option in filterOptions"
        :key="option.id"
        size="compact"
        :selected="selectedFilter === option.id"
        :data-test="`subagent-filter-${option.id}`"
        @click="selectedFilter = option.id"
      >
        {{ option.label }} {{ option.count }}
      </KxChipButton>
    </KxChipGroup>

    <KxEmptyState v-if="rows.length === 0" class="subagent-empty" compact>
      {{ t("subagents.empty") }}
    </KxEmptyState>
    <KxEmptyState v-else-if="filteredRows.length === 0" class="subagent-empty" compact>
      {{ t("subagents.emptyFiltered") }}
    </KxEmptyState>
    <div v-else class="subagent-list">
      <article
        v-for="row in filteredRows"
        :key="row.agent.id"
        class="card subagent-card"
        :data-test="`subagent-card-${row.agent.id}`"
      >
        <header class="subagent-card-header">
          <span class="subagent-label" :class="roleClass(row.agent.role)">
            {{ row.label }}
          </span>
          <span class="subagent-role">{{ row.agent.role }}</span>
          <KxBadge class="subagent-status" :tone="needsAttention(row) ? 'warning' : 'neutral'">
            {{ row.agent.status }}
          </KxBadge>
        </header>

        <dl class="subagent-meta">
          <div class="subagent-meta-row">
            <dt>{{ t("subagents.task") }}</dt>
            <dd v-if="row.task">
              <span class="subagent-task-title">{{ row.task.title }}</span>
              <KxBadge class="subagent-task-state" tone="info">
                {{ row.task.state }}
              </KxBadge>
            </dd>
            <dd v-else>{{ t("subagents.noTask") }}</dd>
          </div>
          <div v-if="row.task?.error" class="subagent-meta-row subagent-error">
            <dt>{{ t("subagents.error") }}</dt>
            <dd>{{ row.task.error }}</dd>
          </div>
        </dl>

        <div
          v-if="row.task && (canRetry(row.task) || canCancel(row.task))"
          class="subagent-actions"
        >
          <KxButton
            v-if="canRetry(row.task)"
            size="xs"
            variant="default"
            :data-test="`subagent-retry-${row.agent.id}`"
            @click="retryTask(row)"
          >
            {{ t("subagents.retry") }}
          </KxButton>
          <KxButton
            v-if="canCancel(row.task)"
            size="xs"
            variant="default"
            :data-test="`subagent-cancel-${row.agent.id}`"
            @click="cancelTask(row)"
          >
            {{ t("subagents.cancel") }}
          </KxButton>
        </div>
      </article>
    </div>
  </section>
</template>

<style scoped>
.subagent-panel {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  max-width: 100%;
  min-height: 0;
  overflow: hidden;
}
.subagent-summary {
  box-sizing: border-box;
  padding: 10px 12px;
  border-bottom: 1px solid var(--app-border-color, #e5e7eb);
  color: var(--app-text-color-2, #475569);
  font-size: 12px;
  font-weight: 600;
}
.subagent-filters {
  box-sizing: border-box;
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color, #e5e7eb);
}
.subagent-list {
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 10px 12px;
}
.subagent-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
}
.subagent-card-header {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.subagent-label {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 22px;
  height: 18px;
  padding: 0 5px;
  border-radius: 4px;
  color: white;
  font-size: 10px;
  font-weight: 700;
  line-height: 18px;
}
.subagent-role--planner {
  background: #0077cc;
}
.subagent-role--worker {
  background: #22a06b;
}
.subagent-role--reviewer {
  background: #7c3aed;
}
.subagent-role {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--app-text-color, #111827);
  font-size: 12px;
  font-weight: 700;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.subagent-status {
  flex-shrink: 0;
  text-transform: lowercase;
}
.subagent-meta {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 0;
}
.subagent-meta-row {
  display: grid;
  grid-template-columns: 52px minmax(0, 1fr);
  gap: 8px;
  min-width: 0;
  font-size: 12px;
}
.subagent-meta-row dt {
  color: var(--app-text-color-3, #64748b);
}
.subagent-meta-row dd {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  margin: 0;
  color: var(--app-text-color, #111827);
}
.subagent-task-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.subagent-task-state {
  flex-shrink: 0;
}
.subagent-error dd {
  color: var(--app-error-color, #cc3333);
}
.subagent-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
}
.subagent-empty {
  box-sizing: border-box;
  width: calc(100% - 24px);
  margin: 12px;
  font-size: 12px;
}
</style>

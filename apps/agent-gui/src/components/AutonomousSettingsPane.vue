<script setup lang="ts">
import { useAutonomousStore } from "@/stores/autonomous";
import { storeToRefs } from "pinia";
import type { AutonomousTaskView } from "@/types";
import SettingsStatusTag from "@/components/ui/SettingsStatusTag.vue";
import KxButton from "@/components/ui/KxButton.vue";

const { t } = useI18n();
const store = useAutonomousStore();
const { tasks, selectedTask, checkpoints, loading, error } = storeToRefs(store);

const busyTaskId = ref<string | null>(null);

const stateTone: Record<string, "success" | "warning" | "error" | "info"> = {
  active: "success",
  paused: "warning",
  completed: "info",
  cancelled: "error",
  failed: "error"
};

function formatDate(value: string): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short"
  }).format(new Date(value));
}

function truncateGoal(goal: string, maxLength = 80): string {
  return goal.length > maxLength ? `${goal.slice(0, maxLength)}…` : goal;
}

async function handlePause(taskId: string): Promise<void> {
  busyTaskId.value = taskId;
  try {
    await store.pauseTask(taskId);
  } finally {
    busyTaskId.value = null;
  }
}

async function handleResume(taskId: string): Promise<void> {
  busyTaskId.value = taskId;
  try {
    await store.resumeTask(taskId);
  } finally {
    busyTaskId.value = null;
  }
}

async function handleCancel(task: AutonomousTaskView): Promise<void> {
  busyTaskId.value = task.autonomous_task_id;
  try {
    await store.cancelTask(task.autonomous_task_id, task.current_session_id);
  } finally {
    busyTaskId.value = null;
  }
}

function selectTask(taskId: string): void {
  if (selectedTask.value?.autonomous_task_id === taskId) {
    store.selectTask(null);
    return;
  }
  store.selectTask(taskId);
  store.fetchCheckpoints(taskId);
}

onMounted(() => {
  store.fetchTasks();
});
</script>

<template>
  <div class="autonomous-pane" data-test="autonomous-pane">
    <div v-if="error" class="alert alert--danger" data-test="autonomous-error">
      {{ error }}
    </div>

    <div v-if="loading && tasks.length === 0" class="empty-state">
      <span class="spinner" />
      {{ t("common.loading") }}
    </div>

    <div v-else-if="tasks.length === 0" class="empty-state" data-test="autonomous-empty">
      {{ t("settings.autonomousEmpty") }}
    </div>

    <template v-else>
      <div class="autonomous-pane__list" data-test="autonomous-task-list">
        <div
          v-for="task in tasks"
          :key="task.autonomous_task_id"
          class="card autonomous-pane__card"
          :class="{
            'autonomous-pane__card--selected':
              selectedTask?.autonomous_task_id === task.autonomous_task_id
          }"
          :data-test="`autonomous-task-${task.autonomous_task_id}`"
          @click="selectTask(task.autonomous_task_id)"
        >
          <div class="autonomous-pane__card-header">
            <span class="autonomous-pane__goal">{{ truncateGoal(task.goal) }}</span>
            <SettingsStatusTag :tone="stateTone[task.state] ?? 'info'">
              {{ task.state }}
            </SettingsStatusTag>
          </div>

          <div class="autonomous-pane__card-meta">
            <span
              >{{ t("settings.autonomousSessions") }}: {{ task.session_count }}/{{
                task.max_sessions
              }}</span
            >
            <span>{{ formatDate(task.created_at) }}</span>
          </div>

          <div class="autonomous-pane__card-actions">
            <KxButton
              v-if="task.state === 'active'"
              size="sm"
              :disabled="busyTaskId === task.autonomous_task_id"
              data-test="autonomous-pause-btn"
              @click.stop="handlePause(task.autonomous_task_id)"
            >
              {{ t("settings.autonomousPause") }}
            </KxButton>
            <KxButton
              v-if="task.state === 'paused'"
              size="sm"
              :disabled="busyTaskId === task.autonomous_task_id"
              data-test="autonomous-resume-btn"
              @click.stop="handleResume(task.autonomous_task_id)"
            >
              {{ t("settings.autonomousResume") }}
            </KxButton>
            <KxButton
              v-if="task.state === 'active' || task.state === 'paused'"
              variant="danger-ghost"
              size="sm"
              :disabled="busyTaskId === task.autonomous_task_id"
              data-test="autonomous-cancel-btn"
              @click.stop="handleCancel(task)"
            >
              {{ t("settings.autonomousCancel") }}
            </KxButton>
          </div>
        </div>
      </div>

      <div v-if="selectedTask" class="autonomous-pane__detail" data-test="autonomous-detail">
        <h3 class="autonomous-pane__detail-title">{{ selectedTask.goal }}</h3>

        <dl class="autonomous-pane__detail-meta">
          <dt>{{ t("settings.autonomousState") }}</dt>
          <dd>
            <SettingsStatusTag :tone="stateTone[selectedTask.state] ?? 'info'">
              {{ selectedTask.state }}
            </SettingsStatusTag>
          </dd>
          <dt>{{ t("settings.autonomousSessions") }}</dt>
          <dd>{{ selectedTask.session_count }} / {{ selectedTask.max_sessions }}</dd>
          <dt>{{ t("settings.autonomousCreated") }}</dt>
          <dd>{{ formatDate(selectedTask.created_at) }}</dd>
          <dt>{{ t("settings.autonomousUpdated") }}</dt>
          <dd>{{ formatDate(selectedTask.updated_at) }}</dd>
        </dl>

        <h4 v-if="checkpoints.length > 0">{{ t("settings.autonomousCheckpoints") }}</h4>
        <div
          v-if="checkpoints.length > 0"
          class="autonomous-pane__checkpoints"
          data-test="autonomous-checkpoints"
        >
          <div
            v-for="checkpoint in checkpoints"
            :key="checkpoint.checkpoint_id"
            class="card autonomous-pane__checkpoint"
          >
            <div class="autonomous-pane__checkpoint-header">
              <span class="tag"
                >{{ t("settings.autonomousSession") }} {{ checkpoint.session_index + 1 }}</span
              >
              <span class="autonomous-pane__checkpoint-reason">{{ checkpoint.end_reason }}</span>
            </div>
            <div class="autonomous-pane__checkpoint-body">
              <div v-if="checkpoint.completed_items.length > 0">
                <strong>{{ t("settings.autonomousCompleted") }}:</strong>
                <ul>
                  <li v-for="(item, index) in checkpoint.completed_items" :key="index">
                    {{ item }}
                  </li>
                </ul>
              </div>
              <div v-if="checkpoint.remaining_items.length > 0">
                <strong>{{ t("settings.autonomousRemaining") }}:</strong>
                <ul>
                  <li v-for="(item, index) in checkpoint.remaining_items" :key="index">
                    {{ item }}
                  </li>
                </ul>
              </div>
              <div v-if="checkpoint.git_sha" class="autonomous-pane__checkpoint-sha">
                git: <code>{{ checkpoint.git_sha.slice(0, 8) }}</code>
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.autonomous-pane {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.autonomous-pane__list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.autonomous-pane__card {
  cursor: pointer;
  padding: 12px 16px;
  transition: border-color 0.15s;
}
.autonomous-pane__card:hover {
  border-color: var(--app-primary-color);
}
.autonomous-pane__card--selected {
  border-color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 6%, var(--app-card-color));
}
.autonomous-pane__card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
}
.autonomous-pane__goal {
  font-weight: 600;
  font-size: var(--app-text-base);
  color: var(--app-text-color);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.autonomous-pane__card-meta {
  display: flex;
  gap: 16px;
  margin-top: 6px;
  font-size: var(--app-text-sm);
  color: var(--app-text-color-3);
}
.autonomous-pane__card-actions {
  display: flex;
  gap: 6px;
  margin-top: 8px;
}
.autonomous-pane__detail {
  border-top: 1px solid var(--app-border-color);
  padding-top: 16px;
}
.autonomous-pane__detail-title {
  margin: 0 0 12px;
  font-size: var(--app-text-lg);
  font-weight: 650;
  color: var(--app-text-color);
  overflow-wrap: break-word;
}
.autonomous-pane__detail-meta {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 4px 16px;
  font-size: var(--app-text-sm);
  margin-bottom: 16px;
}
.autonomous-pane__detail-meta dt {
  font-weight: 600;
  color: var(--app-text-color-2);
}
.autonomous-pane__detail-meta dd {
  margin: 0;
  color: var(--app-text-color);
}
.autonomous-pane__checkpoints {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.autonomous-pane__checkpoint {
  padding: 10px 14px;
}
.autonomous-pane__checkpoint-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.autonomous-pane__checkpoint-reason {
  font-size: var(--app-text-sm);
  color: var(--app-text-color-3);
}
.autonomous-pane__checkpoint-body {
  font-size: var(--app-text-sm);
  color: var(--app-text-color-2);
}
.autonomous-pane__checkpoint-body ul {
  margin: 4px 0 8px;
  padding-left: 20px;
}
.autonomous-pane__checkpoint-body li {
  margin-bottom: 2px;
}
.autonomous-pane__checkpoint-sha {
  margin-top: 6px;
  font-size: var(--app-text-xs, 11px);
  color: var(--app-text-color-3);
}
.autonomous-pane__checkpoint-sha code {
  font-family: var(--app-font-mono, monospace);
  background: var(--app-hover-color);
  padding: 1px 5px;
  border-radius: var(--app-radius-sm);
}
</style>

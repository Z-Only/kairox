<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useSessionStore } from "@/stores/session";
import { commands } from "@/generated/commands";
import type {
  TrajectoryMetaResponse,
  TrajectoryStepResponse,
  TrajectoryOutcome
} from "@/generated/commands";

const { t } = useI18n();
const session = useSessionStore();

const trajectories = ref<TrajectoryMetaResponse[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

const expandedTrajectoryId = ref<string | null>(null);
const steps = ref<TrajectoryStepResponse[]>([]);
const stepsLoading = ref(false);

const expandedInputs = ref<Set<number>>(new Set());
const expandedObservations = ref<Set<number>>(new Set());

const TRUNCATE_LENGTH = 120;

const outcomeBadgeClass: Record<TrajectoryOutcome, string> = {
  success: "badge--success",
  failed: "badge--failed",
  cancelled: "badge--cancelled",
  in_progress: "badge--in-progress"
};

function outcomeClass(outcome: string): string {
  return outcomeBadgeClass[outcome as TrajectoryOutcome] ?? "badge--in-progress";
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function formatTime(iso: string): string {
  try {
    return new Date(iso).toLocaleTimeString();
  } catch {
    return iso;
  }
}

function formatTimeRange(started: string, completed: string | null): string {
  const start = formatTime(started);
  if (!completed) return start;
  return `${start} - ${formatTime(completed)}`;
}

function truncate(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text;
  return text.slice(0, maxLen) + "…";
}

function toggleInput(stepIndex: number) {
  const next = new Set(expandedInputs.value);
  if (next.has(stepIndex)) {
    next.delete(stepIndex);
  } else {
    next.add(stepIndex);
  }
  expandedInputs.value = next;
}

function toggleObservation(stepIndex: number) {
  const next = new Set(expandedObservations.value);
  if (next.has(stepIndex)) {
    next.delete(stepIndex);
  } else {
    next.add(stepIndex);
  }
  expandedObservations.value = next;
}

async function fetchTrajectories() {
  const sessionId = session.currentSessionId;
  if (!sessionId) {
    trajectories.value = [];
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    const result = await commands.listTrajectories(sessionId);
    if (result.status === "ok") {
      trajectories.value = result.data;
    } else {
      error.value = result.error;
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function toggleTrajectory(trajectoryId: string) {
  if (expandedTrajectoryId.value === trajectoryId) {
    expandedTrajectoryId.value = null;
    steps.value = [];
    expandedInputs.value = new Set();
    expandedObservations.value = new Set();
    return;
  }

  expandedTrajectoryId.value = trajectoryId;
  stepsLoading.value = true;
  expandedInputs.value = new Set();
  expandedObservations.value = new Set();
  try {
    const result = await commands.getTrajectorySteps(trajectoryId);
    if (result.status === "ok") {
      steps.value = result.data;
    } else {
      steps.value = [];
    }
  } catch {
    steps.value = [];
  } finally {
    stepsLoading.value = false;
  }
}

async function exportTrajectory(trajectoryId: string, event: Event) {
  event.stopPropagation();
  try {
    const result = await commands.exportTrajectory(trajectoryId);
    if (result.status === "ok") {
      await navigator.clipboard.writeText(result.data);
    }
  } catch {
    // silent
  }
}

watch(() => session.currentSessionId, fetchTrajectories, { immediate: true });
</script>

<template>
  <div class="trajectory-viewer" data-test="trajectory-viewer">
    <KxEmptyState v-if="!session.currentSessionId" class="trajectory-empty" compact>
      {{ t("trajectory.noSession") }}
    </KxEmptyState>

    <KxEmptyState v-else-if="loading" class="trajectory-empty" compact>
      {{ t("trajectory.loading") }}
    </KxEmptyState>

    <KxEmptyState v-else-if="error" class="trajectory-empty" compact>
      {{ error }}
    </KxEmptyState>

    <KxEmptyState v-else-if="trajectories.length === 0" class="trajectory-empty" compact>
      {{ t("trajectory.empty") }}
    </KxEmptyState>

    <div v-else class="trajectory-list" :style="{ overflowY: 'auto' }">
      <div
        v-for="traj in trajectories"
        :key="traj.trajectory_id"
        class="trajectory-card"
        data-test="trajectory-card"
        @click="toggleTrajectory(traj.trajectory_id)"
      >
        <div class="trajectory-card-header">
          <span class="trajectory-task-id">{{ traj.task_id }}</span>
          <span class="trajectory-badge" :class="outcomeClass(traj.outcome)">
            {{ traj.outcome }}
          </span>
        </div>
        <div class="trajectory-card-meta">
          <span class="trajectory-step-count">
            {{ t("trajectory.stepCount", { count: traj.step_count }) }}
          </span>
          <span class="trajectory-time-range">
            {{ formatTimeRange(traj.started_at, traj.completed_at) }}
          </span>
        </div>
        <div class="trajectory-card-actions">
          <KxButton
            size="xs"
            variant="default"
            data-test="trajectory-export"
            @click="exportTrajectory(traj.trajectory_id, $event)"
          >
            {{ t("trajectory.export") }}
          </KxButton>
        </div>

        <div
          v-if="expandedTrajectoryId === traj.trajectory_id"
          class="trajectory-steps"
          @click.stop
        >
          <KxEmptyState v-if="stepsLoading" compact>
            {{ t("trajectory.loadingSteps") }}
          </KxEmptyState>
          <KxEmptyState v-else-if="steps.length === 0" compact>
            {{ t("trajectory.noSteps") }}
          </KxEmptyState>
          <div
            v-for="step in steps"
            v-else
            :key="step.step_index"
            class="trajectory-step"
            data-test="trajectory-step"
          >
            <div class="step-header">
              <span class="step-index">#{{ step.step_index }}</span>
              <span class="step-action">{{ step.action }}</span>
              <span class="step-duration">{{ formatDuration(step.duration_ms) }}</span>
              <span class="step-timestamp">{{ formatTime(step.timestamp) }}</span>
            </div>
            <div class="step-body">
              <div class="step-field" @click="toggleInput(step.step_index)">
                <span class="step-field-label">{{ t("trajectory.input") }}</span>
                <pre class="step-field-value">{{
                  expandedInputs.has(step.step_index)
                    ? step.action_input
                    : truncate(step.action_input, TRUNCATE_LENGTH)
                }}</pre>
              </div>
              <div class="step-field" @click="toggleObservation(step.step_index)">
                <span class="step-field-label">{{ t("trajectory.observation") }}</span>
                <pre class="step-field-value">{{
                  expandedObservations.has(step.step_index)
                    ? step.observation
                    : truncate(step.observation, TRUNCATE_LENGTH)
                }}</pre>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.trajectory-viewer {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  max-width: 100%;
  min-height: 0;
}
.trajectory-list {
  box-sizing: border-box;
  flex: 1;
  min-width: 0;
  max-width: 100%;
  min-height: 0;
  overflow-x: hidden;
}
.trajectory-card {
  box-sizing: border-box;
  padding: 10px 12px;
  border-bottom: 1px solid var(--app-border-color);
  cursor: pointer;
}
.trajectory-card:hover {
  background: var(--app-hover-color);
}
.trajectory-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.trajectory-task-id {
  font-size: 13px;
  font-weight: 500;
  color: var(--app-text-color);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}
.trajectory-badge {
  flex-shrink: 0;
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 500;
}
.badge--success {
  background: var(--app-success-bg, #dcfce7);
  color: var(--app-success-color, #166534);
}
.badge--failed {
  background: var(--app-error-bg, #fee2e2);
  color: var(--app-error-color, #991b1b);
}
.badge--cancelled {
  background: var(--app-warning-bg, #fef9c3);
  color: var(--app-warning-color, #854d0e);
}
.badge--in-progress {
  background: var(--app-info-bg, #dbeafe);
  color: var(--app-info-color, #1e40af);
}
.trajectory-card-meta {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 4px;
  font-size: 11px;
  color: var(--app-text-color-3);
}
.trajectory-card-actions {
  margin-top: 6px;
}
.trajectory-steps {
  margin-top: 8px;
  border-top: 1px solid var(--app-border-color);
  padding-top: 8px;
}
.trajectory-step {
  padding: 6px 0;
  border-bottom: 1px solid var(--app-border-color);
}
.trajectory-step:last-child {
  border-bottom: none;
}
.step-header {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}
.step-index {
  font-weight: 600;
  color: var(--app-text-color-3);
  min-width: 28px;
}
.step-action {
  font-weight: 500;
  color: var(--app-text-color);
}
.step-duration {
  margin-left: auto;
  color: var(--app-text-color-3);
  font-size: 11px;
}
.step-timestamp {
  color: var(--app-text-color-3);
  font-size: 11px;
}
.step-body {
  margin-top: 4px;
}
.step-field {
  cursor: pointer;
  padding: 2px 0;
}
.step-field-label {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--app-text-color-3);
  letter-spacing: 0.5px;
}
.step-field-value {
  font-size: 12px;
  color: var(--app-text-color-2);
  margin: 2px 0 0;
  white-space: pre-wrap;
  overflow-wrap: break-word;
  font-family: var(--app-font-mono);
}
.trajectory-empty {
  box-sizing: border-box;
  width: calc(100% - 24px);
  margin: 12px;
  font-size: 12px;
}
</style>

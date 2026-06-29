<script setup lang="ts">
import { useI18n } from "vue-i18n";
import TraceEntry from "./TraceEntry.vue";
import TaskSteps from "./TaskSteps.vue";
import SubagentPanel from "./SubagentPanel.vue";
import MemoryBrowser from "./MemoryBrowser.vue";
import TrajectoryViewer from "./TrajectoryViewer.vue";
import GitReviewSidebarPanel from "./GitReviewSidebarPanel.vue";
import { traceState } from "../composables/useTraceStore";
import { useWorkspaceUiStore } from "@/stores/workspaceUi";
import { useSessionStore } from "@/stores/session";
import { useToast } from "@/composables/useToast";
import { commands } from "@/generated/commands";
import type { TraceEntryData } from "../types/trace";

type TraceStatusFilter = "all" | "active" | "failed" | "done";
type TraceKindFilter = "all" | "tool" | "permission" | "memory";

const { t } = useI18n();
const workspaceUi = useWorkspaceUiStore();
const session = useSessionStore();
const toast = useToast();
const { rightPanelTab } = storeToRefs(workspaceUi);
const selectedTraceFilter = ref<TraceStatusFilter>("all");
const selectedTraceKindFilter = ref<TraceKindFilter>("all");
const traceSearchQuery = ref("");
const copyingDiagnostics = ref(false);
const normalizedTraceSearchQuery = computed(() => traceSearchQuery.value.trim().toLowerCase());
const activeTraceStatuses = new Set(["pending", "running"]);
const canOpenChangesTab = computed(() => {
  const sessionInfo = session.currentSessionInfo;
  return Boolean(sessionInfo?.project_id || sessionInfo?.worktree_path);
});

watch(
  canOpenChangesTab,
  (canOpen) => {
    if (canOpen || rightPanelTab.value !== "changes") return;
    workspaceUi.clearGitReview();
    rightPanelTab.value = "trace";
  },
  { immediate: true }
);

function effectiveTraceStatus(entry: TraceEntryData) {
  return entry.status === "completed" && entry.exitCode != null && entry.exitCode !== 0
    ? "failed"
    : entry.status;
}

function traceMatchesFilter(entry: TraceEntryData, filter: TraceStatusFilter) {
  const status = effectiveTraceStatus(entry);
  switch (filter) {
    case "active":
      return activeTraceStatuses.has(status);
    case "failed":
      return status === "failed";
    case "done":
      return status === "completed";
    default:
      return true;
  }
}

function traceMatchesKind(entry: TraceEntryData, filter: TraceKindFilter) {
  return filter === "all" || entry.kind === filter;
}

function traceMatchesSearch(entry: TraceEntryData, query: string) {
  if (!query) return true;
  return [
    entry.id,
    entry.kind,
    entry.status,
    entry.title,
    entry.toolId,
    entry.input,
    entry.outputPreview,
    entry.outputFull,
    entry.reason,
    entry.scope,
    entry.content
  ].some((value) => value?.toLowerCase().includes(query));
}

const traceFilterOptions = computed<{ id: TraceStatusFilter; label: string; count: number }[]>(
  () => [
    { id: "all", label: t("trace.filterAll"), count: traceState.entries.length },
    {
      id: "active",
      label: t("trace.filterActive"),
      count: traceState.entries.filter((entry) => traceMatchesFilter(entry, "active")).length
    },
    {
      id: "failed",
      label: t("trace.filterFailed"),
      count: traceState.entries.filter((entry) => traceMatchesFilter(entry, "failed")).length
    },
    {
      id: "done",
      label: t("trace.filterDone"),
      count: traceState.entries.filter((entry) => traceMatchesFilter(entry, "done")).length
    }
  ]
);

const traceKindOptions = computed<{ id: TraceKindFilter; label: string }[]>(() => [
  { id: "all", label: t("trace.filterKindAll") },
  { id: "tool", label: t("trace.filterKindTools") },
  { id: "permission", label: t("trace.filterKindPermissions") },
  { id: "memory", label: t("trace.filterKindMemories") }
]);

const visibleTraceEntries = computed(() =>
  traceState.entries.filter(
    (entry) =>
      traceMatchesFilter(entry, selectedTraceFilter.value) &&
      traceMatchesKind(entry, selectedTraceKindFilter.value) &&
      traceMatchesSearch(entry, normalizedTraceSearchQuery.value)
  )
);

async function copySessionDiagnostics() {
  const sessionId = session.currentSessionId;
  if (!sessionId || copyingDiagnostics.value) return;

  copyingDiagnostics.value = true;
  try {
    const result = await commands.exportSessionDiagnostics(sessionId);
    if (result.status === "error") {
      throw new Error(result.error);
    }
    await navigator.clipboard.writeText(JSON.stringify(result.data));
    toast.success(t("trace.diagnosticsCopied"));
  } catch (error) {
    toast.error(t("trace.diagnosticsCopyFailed", { error: String(error) }));
  } finally {
    copyingDiagnostics.value = false;
  }
}

async function openChangesTab(): Promise<void> {
  if (!canOpenChangesTab.value) return;

  const sessionInfo = session.currentSessionInfo;
  const projectId = sessionInfo?.project_id ?? null;
  if (!sessionInfo?.id && !projectId) return;

  await workspaceUi.openGitReview({
    sessionId: sessionInfo?.id ?? null,
    projectId
  });
}
</script>

<template>
  <section class="trace-timeline" data-test="trace-timeline">
    <header class="trace-header">
      <!-- Hand-rolled tab strip rather than NTabs because the existing
           unit tests assert against `.tab-group button` selectors and the
           panel below is a 3-way switch over heterogeneous components
           (TraceEntry list / TaskSteps / MemoryBrowser) — a tab-pane
           teleport approach would force every panel to render, which we
           don't want here. The buttons use Kairox button chrome
           for consistent theming without touching the tests' active-class
           assertion. -->
      <div class="tab-group">
        <KxButton
          size="sm"
          :variant="rightPanelTab === 'trace' ? 'primary' : 'default'"
          :class="{ active: rightPanelTab === 'trace' }"
          @click="rightPanelTab = 'trace'"
        >
          {{ t("trace.tabTrace") }}
        </KxButton>
        <KxButton
          size="sm"
          :variant="rightPanelTab === 'tasks' ? 'primary' : 'default'"
          :class="{ active: rightPanelTab === 'tasks' }"
          data-test="trace-tab-tasks"
          @click="rightPanelTab = 'tasks'"
        >
          {{ t("trace.tabTasks") }}
        </KxButton>
        <KxButton
          size="sm"
          :variant="rightPanelTab === 'memory' ? 'primary' : 'default'"
          :class="{ active: rightPanelTab === 'memory' }"
          data-test="trace-tab-memory"
          @click="rightPanelTab = 'memory'"
        >
          {{ t("trace.tabMemory") }}
        </KxButton>
        <KxButton
          size="sm"
          :variant="rightPanelTab === 'subagents' ? 'primary' : 'default'"
          :class="{ active: rightPanelTab === 'subagents' }"
          data-test="trace-tab-subagents"
          @click="rightPanelTab = 'subagents'"
        >
          {{ t("trace.tabSubagents") }}
        </KxButton>
        <KxButton
          v-if="canOpenChangesTab"
          size="sm"
          :variant="rightPanelTab === 'changes' ? 'primary' : 'default'"
          :class="{ active: rightPanelTab === 'changes' }"
          data-test="trace-tab-changes"
          @click="openChangesTab"
        >
          {{ t("trace.tabChanges") }}
        </KxButton>
        <KxButton
          size="sm"
          :variant="rightPanelTab === 'trajectory' ? 'primary' : 'default'"
          :class="{ active: rightPanelTab === 'trajectory' }"
          data-test="trace-tab-trajectory"
          @click="rightPanelTab = 'trajectory'"
        >
          {{ t("trace.tabTrajectory") }}
        </KxButton>
      </div>
      <div class="trace-header-actions">
        <KxIconButton
          size="sm"
          variant="default"
          :label="t('trace.copyDiagnostics')"
          :title="t('trace.copyDiagnostics')"
          :disabled="!session.currentSessionId"
          :busy="copyingDiagnostics"
          data-test="trace-copy-diagnostics"
          @click="copySessionDiagnostics"
        >
          <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
            <path
              d="M6 2h7.5L17 5.5V16a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2Zm7 1.8V6h2.2L13 3.8ZM6 4v12h9V8h-4V4H6Zm1.5 6h6v1.4h-6V10Zm0 3h4.5v1.4H7.5V13Z"
            />
          </svg>
        </KxIconButton>
      </div>
    </header>
    <div v-if="rightPanelTab === 'trace'" class="trace-entries" :style="{ overflowY: 'auto' }">
      <div v-if="traceState.entries.length > 0" class="trace-filters" data-test="trace-filters">
        <KxChipGroup
          class="trace-status-filters"
          :aria-label="t('trace.statusFiltersAria')"
          data-test="trace-status-filters"
        >
          <KxChipButton
            v-for="option in traceFilterOptions"
            :key="option.id"
            size="compact"
            :selected="selectedTraceFilter === option.id"
            :data-test="`trace-filter-${option.id}`"
            @click="selectedTraceFilter = option.id"
          >
            {{ option.label }} {{ option.count }}
          </KxChipButton>
        </KxChipGroup>
        <KxSelect
          v-model="selectedTraceKindFilter"
          class="trace-kind-select"
          size="compact"
          :aria-label="t('trace.kindSelectAria')"
          data-test="trace-kind-select"
        >
          <option v-for="option in traceKindOptions" :key="option.id" :value="option.id">
            {{ option.label }}
          </option>
        </KxSelect>
        <div class="trace-search-wrapper">
          <svg class="trace-search-icon" viewBox="0 0 20 20" aria-hidden="true" focusable="false">
            <path
              d="M8.5 3a5.5 5.5 0 0 1 4.38 8.82l3.65 3.66-1.06 1.06-3.66-3.65A5.5 5.5 0 1 1 8.5 3Zm0 1.5a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z"
            />
          </svg>
          <KxInput
            v-model="traceSearchQuery"
            type="search"
            size="compact"
            :aria-label="t('trace.searchAria')"
            :placeholder="t('trace.searchPlaceholder')"
            class="trace-search-input"
            data-test="trace-search-input"
          />
        </div>
      </div>
      <div class="density-toolbar">
        <span class="density-label">{{ t("trace.densityLabel") }}</span>
        <KxButton
          v-for="d in ['L1', 'L2', 'L3'] as const"
          :key="d"
          size="xs"
          :variant="traceState.density === d ? 'primary' : 'default'"
          class="density-btn"
          :class="{
            'density-btn--active': traceState.density === d,
            active: traceState.density === d
          }"
          @click="traceState.density = d"
        >
          {{ d }}
        </KxButton>
      </div>
      <TraceEntry
        v-for="entry in visibleTraceEntries"
        :key="entry.id"
        :entry="entry"
        :density="traceState.density"
      />
      <KxEmptyState v-if="traceState.entries.length === 0" class="trace-empty" compact>
        {{ t("trace.emptyTrace") }}
      </KxEmptyState>
      <KxEmptyState v-else-if="visibleTraceEntries.length === 0" class="trace-empty" compact>
        {{ t("trace.emptyFilteredTrace") }}
      </KxEmptyState>
    </div>
    <TaskSteps v-if="rightPanelTab === 'tasks'" />
    <MemoryBrowser v-if="rightPanelTab === 'memory'" />
    <SubagentPanel v-if="rightPanelTab === 'subagents'" />
    <GitReviewSidebarPanel v-if="rightPanelTab === 'changes'" />
    <TrajectoryViewer v-if="rightPanelTab === 'trajectory'" />
  </section>
</template>

<style scoped>
.trace-timeline {
  display: flex;
  flex-direction: column;
  min-width: 0;
  max-width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--app-panel-color);
}
.trace-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  min-height: 36px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color);
  background: var(--app-panel-color);
}
.tab-group {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  max-width: 100%;
  min-width: 0;
}
.trace-header-actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  padding-left: 8px;
}
.trace-entries {
  box-sizing: border-box;
  flex: 1;
  min-width: 0;
  max-width: 100%;
  min-height: 0;
  overflow-x: hidden;
}
.trace-filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color);
  background: var(--app-panel-color);
}
.trace-status-filters {
  flex: 1 1 auto;
}
.trace-kind-select {
  flex: 0 1 140px;
}
.trace-search-wrapper {
  position: relative;
  flex: 1 1 180px;
  display: flex;
  align-items: center;
}
.trace-search-icon {
  position: absolute;
  left: 8px;
  width: 14px;
  height: 14px;
  fill: var(--app-text-color-3);
  pointer-events: none;
  z-index: 1;
}
.trace-search-input {
  flex: 1;
  padding-left: 26px;
}
.density-toolbar {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 12px;
  border-bottom: 1px solid var(--app-border-color);
  background: var(--app-panel-color);
}
.density-label {
  font-size: 11px;
  color: var(--app-text-color-3);
  margin-right: 2px;
}
.density-btn {
  min-width: 30px;
  font-size: 11px;
}
.trace-empty {
  box-sizing: border-box;
  width: calc(100% - 24px);
  margin: 12px;
  font-size: 12px;
}
</style>

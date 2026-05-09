<script setup lang="ts">
import { useI18n } from "vue-i18n";
import TraceEntry from "./TraceEntry.vue";
import TaskSteps from "./TaskSteps.vue";
import MemoryBrowser from "./MemoryBrowser.vue";
import { traceState } from "../composables/useTraceStore";

const { t } = useI18n();
const rightPanelTab = ref<"trace" | "tasks" | "memory">("trace");
</script>

<template>
  <section class="trace-timeline" data-test="trace-timeline">
    <header class="trace-header">
      <!-- Hand-rolled tab strip rather than NTabs because the existing
           unit tests assert against `.tab-group button` selectors and the
           panel below is a 3-way switch over heterogeneous components
           (TraceEntry list / TaskSteps / MemoryBrowser) — a tab-pane
           teleport approach would force every panel to render, which we
           don't want here. The buttons use shared CSS utility classes
           for consistent theming without touching the tests' active-class
           assertion. -->
      <div class="tab-group">
        <button
          class="btn btn-sm"
          :class="{ 'btn-primary': rightPanelTab === 'trace', active: rightPanelTab === 'trace' }"
          @click="rightPanelTab = 'trace'"
        >
          {{ t("trace.tabTrace") }}
        </button>
        <button
          class="btn btn-sm"
          :class="{ 'btn-primary': rightPanelTab === 'tasks', active: rightPanelTab === 'tasks' }"
          data-test="trace-tab-tasks"
          @click="rightPanelTab = 'tasks'"
        >
          {{ t("trace.tabTasks") }}
        </button>
        <button
          class="btn btn-sm"
          :class="{ 'btn-primary': rightPanelTab === 'memory', active: rightPanelTab === 'memory' }"
          data-test="trace-tab-memory"
          @click="rightPanelTab = 'memory'"
        >
          {{ t("trace.tabMemory") }}
        </button>
      </div>
    </header>
    <div v-if="rightPanelTab === 'trace'" class="trace-entries" :style="{ overflowY: 'auto' }">
      <div class="density-toolbar">
        <span class="density-label">Detail:</span>
        <button
          v-for="d in ['L1', 'L2', 'L3'] as const"
          :key="d"
          class="density-btn"
          :class="{
            'density-btn--active': traceState.density === d,
            active: traceState.density === d
          }"
          @click="traceState.density = d"
        >
          {{ d }}
        </button>
      </div>
      <TraceEntry
        v-for="entry in traceState.entries"
        :key="entry.id"
        :entry="entry"
        :density="traceState.density"
      />
      <div v-if="traceState.entries.length === 0" class="empty-state empty-hint">
        {{ t("trace.emptyTrace") }}
      </div>
    </div>
    <TaskSteps v-if="rightPanelTab === 'tasks'" />
    <MemoryBrowser v-if="rightPanelTab === 'memory'" />
  </section>
</template>

<style scoped>
.trace-timeline {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.trace-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--app-border-color);
}
.tab-group {
  display: flex;
  gap: 4px;
}
.trace-entries {
  flex: 1;
  min-height: 0;
}
.density-toolbar {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 12px;
  border-bottom: 1px solid var(--app-border-color);
  background: var(--app-card-color);
}
.density-label {
  font-size: 11px;
  color: var(--app-text-color-3);
  margin-right: 2px;
}
.density-btn {
  padding: 1px 8px;
  border: 1px solid var(--app-border-color);
  border-radius: 3px;
  background: transparent;
  cursor: pointer;
  font-size: 11px;
  color: var(--app-text-color-2);
  transition: all 0.15s;
}
.density-btn:hover {
  background: var(--app-hover-color);
  color: var(--app-text-color);
}
.density-btn--active {
  background: var(--app-primary-color);
  color: var(--app-primary-contrast-color);
  border-color: var(--app-primary-color);
}
.empty-hint {
  padding: 12px;
  color: var(--app-text-color-3);
  font-size: 12px;
}
</style>

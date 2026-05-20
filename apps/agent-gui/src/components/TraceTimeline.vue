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
      </div>
    </header>
    <div v-if="rightPanelTab === 'trace'" class="trace-entries" :style="{ overflowY: 'auto' }">
      <div class="density-toolbar">
        <span class="density-label">Detail:</span>
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
        v-for="entry in traceState.entries"
        :key="entry.id"
        :entry="entry"
        :density="traceState.density"
      />
      <KxEmptyState v-if="traceState.entries.length === 0" class="trace-empty" compact>
        {{ t("trace.emptyTrace") }}
      </KxEmptyState>
    </div>
    <TaskSteps v-if="rightPanelTab === 'tasks'" />
    <MemoryBrowser v-if="rightPanelTab === 'memory'" />
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
  box-sizing: border-box;
  flex: 1;
  min-width: 0;
  max-width: 100%;
  min-height: 0;
  overflow-x: hidden;
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

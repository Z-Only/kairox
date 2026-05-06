<script setup lang="ts">
import { ref } from "vue";
import { NScrollbar, NEmpty, NButton } from "naive-ui";
import TraceEntry from "./TraceEntry.vue";
import TaskSteps from "./TaskSteps.vue";
import MemoryBrowser from "./MemoryBrowser.vue";
import { traceState } from "../composables/useTraceStore";

const rightPanelTab = ref<"trace" | "tasks" | "memory">("trace");
</script>

<template>
  <section class="trace-timeline">
    <header class="trace-header">
      <!-- Hand-rolled tab strip rather than NTabs because the existing
           unit tests assert against `.tab-group button` selectors and the
           panel below is a 3-way switch over heterogeneous components
           (TraceEntry list / TaskSteps / MemoryBrowser) — NTabPane's
           teleport semantics would force every panel to render, which we
           don't want here. We do upgrade the buttons themselves to
           NButton so they pick up the NaiveUI theme without touching the
           tests' active-class assertion. -->
      <div class="tab-group">
        <NButton
          size="tiny"
          :type="rightPanelTab === 'trace' ? 'primary' : 'default'"
          :class="{ active: rightPanelTab === 'trace' }"
          @click="rightPanelTab = 'trace'"
        >
          Trace
        </NButton>
        <NButton
          size="tiny"
          :type="rightPanelTab === 'tasks' ? 'primary' : 'default'"
          :class="{ active: rightPanelTab === 'tasks' }"
          @click="rightPanelTab = 'tasks'"
        >
          Tasks
        </NButton>
        <NButton
          size="tiny"
          :type="rightPanelTab === 'memory' ? 'primary' : 'default'"
          :class="{ active: rightPanelTab === 'memory' }"
          @click="rightPanelTab = 'memory'"
        >
          Memory
        </NButton>
      </div>
      <div v-if="rightPanelTab === 'trace'" class="density-toggles">
        <NButton
          v-for="d in ['L1', 'L2', 'L3'] as const"
          :key="d"
          size="tiny"
          :type="traceState.density === d ? 'primary' : 'default'"
          :class="{ active: traceState.density === d }"
          @click="traceState.density = d"
        >
          {{ d }}
        </NButton>
      </div>
    </header>
    <NScrollbar v-if="rightPanelTab === 'trace'" class="trace-entries">
      <TraceEntry
        v-for="entry in traceState.entries"
        :key="entry.id"
        :entry="entry"
        :density="traceState.density"
      />
      <NEmpty
        v-if="traceState.entries.length === 0"
        size="small"
        class="empty-hint"
        description="No trace events yet"
      />
    </NScrollbar>
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
  border-bottom: 1px solid var(--app-border-color, #d7d7d7);
}
.tab-group {
  display: flex;
  gap: 4px;
}
.density-toggles {
  display: flex;
  gap: 4px;
}
.trace-entries {
  flex: 1;
  min-height: 0;
}
.empty-hint {
  padding: 12px;
  color: var(--app-text-disabled-color, #999);
  font-size: 12px;
}
</style>

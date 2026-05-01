<script setup lang="ts">
import TraceEntry from "./TraceEntry.vue";
import { traceState } from "../composables/useTraceStore";
</script>

<template>
  <section class="trace-timeline">
    <header class="trace-header">
      <h2>Trace</h2>
      <div class="density-toggles">
        <button
          v-for="d in ['L1', 'L2', 'L3'] as const"
          :key="d"
          :class="{ active: traceState.density === d }"
          @click="traceState.density = d"
        >
          {{ d }}
        </button>
      </div>
    </header>
    <div class="trace-entries">
      <TraceEntry
        v-for="entry in traceState.entries"
        :key="entry.id"
        :entry="entry"
        :density="traceState.density"
      />
      <p v-if="traceState.entries.length === 0" class="empty-hint">
        No trace events yet
      </p>
    </div>
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
  border-bottom: 1px solid #d7d7d7;
}
.trace-header h2 {
  margin: 0;
  font-size: 14px;
}
.density-toggles {
  display: flex;
  gap: 2px;
}
.density-toggles button {
  padding: 2px 8px;
  border: 1px solid #d7d7d7;
  border-radius: 3px;
  background: white;
  font-size: 11px;
  cursor: pointer;
}
.density-toggles button.active {
  background: #0077cc;
  color: white;
  border-color: #0077cc;
}
.trace-entries {
  flex: 1;
  overflow-y: auto;
}
.empty-hint {
  padding: 12px;
  color: #999;
  font-size: 12px;
}
</style>

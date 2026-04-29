<script setup lang="ts">
import { computed } from "vue";
import { traceLabels, type TraceEvent } from "./TraceTimeline";

const props = defineProps<{ events: TraceEvent[] }>();
const labels = computed(() => traceLabels(props.events));
</script>

<template>
  <section class="trace">
    <h2>Trace</h2>
    <ol>
      <li v-for="label in labels" :key="label">{{ label }}</li>
    </ol>
  </section>
</template>

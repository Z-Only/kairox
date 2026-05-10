<script setup lang="ts">
type ProgressRingState = "normal" | "warning" | "danger" | "success";

const props = withDefaults(
  defineProps<{
    ratio: number;
    label: string;
    state?: ProgressRingState;
  }>(),
  {
    state: "normal"
  }
);

const radius = 18;
const circumference = 2 * Math.PI * radius;
const finiteRatio = computed(() => (Number.isFinite(props.ratio) ? props.ratio : 0));
const clampedRatio = computed(() => Math.min(1, Math.max(0, finiteRatio.value)));
const dashOffset = computed(() => circumference * (1 - clampedRatio.value));
const percentage = computed(() => Math.round(clampedRatio.value * 100));
</script>

<template>
  <div
    class="kx-progress-ring"
    :class="`kx-progress-ring--${state}`"
    data-test="progress-ring"
    role="progressbar"
    :aria-label="label"
    aria-valuemin="0"
    aria-valuemax="100"
    :aria-valuenow="percentage"
  >
    <svg class="kx-progress-ring__svg" viewBox="0 0 48 48" aria-hidden="true">
      <circle class="kx-progress-ring__track" cx="24" cy="24" :r="radius" />
      <circle
        class="kx-progress-ring__value"
        cx="24"
        cy="24"
        :r="radius"
        :stroke-dasharray="circumference"
        :stroke-dashoffset="dashOffset"
      />
    </svg>
    <span class="kx-progress-ring__label">
      <slot>{{ percentage }}%</slot>
    </span>
  </div>
</template>

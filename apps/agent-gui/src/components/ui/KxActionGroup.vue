<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    align?: "start" | "end";
    ariaLabel?: string;
    dataTest?: string;
  }>(),
  {
    align: "end",
    ariaLabel: undefined,
    dataTest: undefined
  }
);
</script>

<template>
  <div
    :class="['kx-action-group', `kx-action-group--${props.align}`]"
    :aria-label="props.ariaLabel"
    :data-test="props.dataTest"
  >
    <slot />
  </div>
</template>

<style scoped>
.kx-action-group {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--app-space-2);
}

.kx-action-group--start {
  justify-content: flex-start;
}

.kx-action-group--end {
  justify-content: flex-end;
}
</style>

<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    ariaLabel?: string;
    dataTest?: string;
  }>(),
  {
    ariaLabel: undefined,
    dataTest: undefined
  }
);
</script>

<template>
  <div class="kx-chip-group" :aria-label="props.ariaLabel" :data-test="props.dataTest">
    <div class="kx-chip-group__chips">
      <slot />
    </div>
    <div v-if="$slots.actions" class="kx-chip-group__actions">
      <slot name="actions" />
    </div>
  </div>
</template>

<style scoped>
.kx-chip-group {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--app-space-2);
}

.kx-chip-group__chips {
  display: flex;
  min-width: 0;
  flex: 1 1 auto;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--app-space-2);
}

.kx-chip-group__actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: var(--app-space-2);
  margin-left: auto;
}
</style>

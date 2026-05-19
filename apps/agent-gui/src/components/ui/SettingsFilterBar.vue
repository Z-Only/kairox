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
  <div class="settings-filter-bar" :aria-label="props.ariaLabel" :data-test="props.dataTest">
    <slot />
  </div>
</template>

<style scoped>
.settings-filter-bar {
  display: grid;
  gap: var(--app-space-3);
  min-width: 0;
}

.settings-filter-bar :deep(form),
.settings-filter-bar :deep(.settings-filter-bar__row) {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--app-space-2);
  min-width: 0;
}
</style>

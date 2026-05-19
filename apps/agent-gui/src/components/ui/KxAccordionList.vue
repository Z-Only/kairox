<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    ariaLabel?: string;
    dataTest?: string;
    role?: string;
  }>(),
  {
    ariaLabel: undefined,
    dataTest: undefined,
    role: undefined
  }
);
</script>

<template>
  <div
    class="kx-accordion-list"
    :role="props.role"
    :aria-label="props.ariaLabel"
    :data-test="props.dataTest"
  >
    <slot />
  </div>
</template>

<style scoped>
.kx-accordion-list {
  display: flex;
  flex-direction: column;
  gap: var(--app-space-1);
  width: 100%;
  min-width: 0;
  margin-top: var(--app-space-2);
}
</style>

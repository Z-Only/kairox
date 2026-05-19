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
  <div class="settings-toolbar" :aria-label="props.ariaLabel" :data-test="props.dataTest">
    <slot />
  </div>
</template>

<style scoped>
.settings-toolbar {
  display: flex;
  flex: none;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--app-space-2);
  min-width: 0;
}
</style>

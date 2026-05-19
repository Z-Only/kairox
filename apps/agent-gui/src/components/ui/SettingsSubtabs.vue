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
  <div
    class="settings-subtabs"
    role="tablist"
    :aria-label="props.ariaLabel"
    :data-test="props.dataTest"
  >
    <slot />
  </div>
</template>

<style scoped>
.settings-subtabs {
  display: flex;
  flex: none;
  flex-wrap: wrap;
  gap: var(--app-space-1);
  min-width: 0;
  border-bottom: 1px solid var(--app-border-color);
}

.settings-subtabs :deep(.sub-tab-btn) {
  padding: 6px 14px;
  border: none;
  border-bottom: 2px solid transparent;
  background: none;
  color: var(--app-text-color-2);
  cursor: pointer;
  font-size: var(--app-text-sm);
  transition:
    color 0.2s,
    border-color 0.2s;
}

.settings-subtabs :deep(.sub-tab-btn[aria-selected="true"]) {
  border-bottom-color: var(--app-primary-color);
  color: var(--app-primary-color);
}

.settings-subtabs :deep(.sub-tab-btn:hover) {
  color: var(--app-primary-color);
}

.settings-subtabs :deep(.sub-tab-btn:focus-visible) {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>

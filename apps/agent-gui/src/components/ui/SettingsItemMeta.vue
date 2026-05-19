<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    as?: "dl" | "div";
    columns?: "auto" | "four";
    compact?: boolean;
    wrapValues?: boolean;
    dataTest?: string;
  }>(),
  {
    as: "dl",
    columns: "auto",
    compact: false,
    wrapValues: false,
    dataTest: undefined
  }
);
</script>

<template>
  <component
    :is="props.as"
    :class="[
      'settings-item-meta',
      `settings-item-meta--${props.columns}`,
      {
        'settings-item-meta--compact': props.compact,
        'settings-item-meta--wrap-values': props.wrapValues
      }
    ]"
    :data-test="props.dataTest"
  >
    <slot />
  </component>
</template>

<style scoped>
.settings-item-meta {
  min-width: 0;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 8px;
  margin: 0;
}

.settings-item-meta--four {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.settings-item-meta--compact {
  gap: 6px 8px;
  color: var(--app-text-color-2);
  font-size: 0.82rem;
}

.settings-item-meta :deep(dt) {
  color: var(--app-text-color-2);
  font-size: 12px;
  font-weight: 600;
}

.settings-item-meta :deep(dd) {
  min-width: 0;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.settings-item-meta--wrap-values :deep(dd),
.settings-item-meta--wrap-values :deep(span) {
  overflow: visible;
  text-overflow: clip;
  white-space: normal;
  overflow-wrap: anywhere;
}

@media (max-width: 760px) {
  .settings-item-meta--four {
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  }
}
</style>

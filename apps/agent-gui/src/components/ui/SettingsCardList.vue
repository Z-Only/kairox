<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    ariaLabel?: string;
    columns?: "single" | "auto";
    dataTest?: string;
    role?: string;
    scroll?: boolean;
    dense?: boolean;
  }>(),
  {
    ariaLabel: undefined,
    columns: "single",
    dataTest: undefined,
    role: "list",
    scroll: true,
    dense: false
  }
);
</script>

<template>
  <div
    :class="[
      'settings-card-list',
      {
        'settings-card-list--scroll': props.scroll,
        'settings-card-list--dense': props.dense,
        'settings-card-list--auto-columns': props.columns === 'auto'
      }
    ]"
    :role="props.role"
    :aria-label="props.ariaLabel"
    :data-test="props.dataTest"
  >
    <slot />
  </div>
</template>

<style scoped>
.settings-card-list {
  display: grid;
  gap: 12px;
  align-content: start;
  align-items: start;
  min-height: 0;
}

.settings-card-list--scroll {
  flex: 1;
  overflow-y: auto;
  padding-right: 4px;
}

.settings-card-list--dense {
  gap: 10px;
}

.settings-card-list--auto-columns {
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 340px), 1fr));
}
</style>

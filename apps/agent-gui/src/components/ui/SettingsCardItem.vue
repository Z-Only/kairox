<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    dataTest?: string;
    role?: string;
    layout?: "split" | "stack";
  }>(),
  {
    dataTest: undefined,
    role: "listitem",
    layout: "split"
  }
);
</script>

<template>
  <article
    :class="['settings-card-item', `settings-card-item--${props.layout}`]"
    :role="props.role"
    :data-test="props.dataTest"
  >
    <slot />
  </article>
</template>

<style scoped>
.settings-card-item {
  min-width: 0;
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  padding: 12px;
  background: var(--app-card-color);
}

.settings-card-item--split {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.settings-card-item--stack {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

@media (max-width: 720px) {
  .settings-card-item--split {
    flex-direction: column;
  }
}
</style>

<script setup lang="ts">
defineOptions({
  inheritAttrs: false
});

const props = withDefaults(
  defineProps<{
    as?: "button" | "div" | "article";
    dataTest?: string;
    role?: string;
  }>(),
  {
    as: "div",
    dataTest: undefined,
    role: undefined
  }
);

const emit = defineEmits<{
  click: [event: MouseEvent];
}>();
</script>

<template>
  <component
    :is="props.as"
    v-bind="$attrs"
    class="kx-accordion-item"
    :type="props.as === 'button' ? 'button' : undefined"
    :role="props.role"
    :data-test="props.dataTest"
    @click="emit('click', $event)"
  >
    <slot />
  </component>
</template>

<style scoped>
.kx-accordion-item {
  display: flex;
  width: 100%;
  min-width: 0;
  align-items: center;
  gap: var(--app-space-2);
  padding: var(--app-space-2) var(--app-space-3);
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-md);
  background: var(--app-card-color);
  color: var(--app-text-color);
  font-size: var(--app-text-sm);
  line-height: 1.35;
  text-align: left;
}

button.kx-accordion-item {
  cursor: pointer;
  transition: background-color 0.15s ease;
}

button.kx-accordion-item:hover {
  background: var(--app-hover-color);
}
</style>

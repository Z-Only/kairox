<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    as?: "span" | "a";
    tone?: "neutral" | "success" | "warning" | "error" | "info" | "muted";
    size?: "sm" | "md";
    dataTest?: string;
  }>(),
  {
    as: "span",
    tone: "neutral",
    size: "md",
    dataTest: undefined
  }
);
</script>

<template>
  <component
    :is="props.as"
    :class="['tag', 'kx-tag', `kx-tag--${props.tone}`, `kx-tag--${props.size}`]"
    :data-test="props.dataTest"
  >
    <slot />
  </component>
</template>

<style scoped>
.kx-tag {
  display: inline-flex;
  align-items: center;
  width: fit-content;
  max-width: 100%;
  gap: 4px;
  border-radius: var(--app-radius-sm);
  font-weight: 650;
  white-space: nowrap;
}

.kx-tag--md {
  padding: 1px 8px;
  font-size: var(--app-text-sm);
  line-height: 1.6;
}

.kx-tag--sm {
  padding: 0 6px;
  font-size: 10px;
  line-height: 1.6;
}

.kx-tag--neutral {
  background: color-mix(in srgb, var(--app-primary-color) 10%, var(--app-card-color));
  color: var(--app-text-color-2);
}

.kx-tag--muted {
  background: color-mix(in srgb, var(--app-muted-surface-color) 80%, var(--app-card-color));
  color: var(--app-text-color-3);
}

.kx-tag--success {
  background: color-mix(in srgb, var(--app-success-color) 15%, var(--app-card-color));
  color: var(--app-success-color);
}

.kx-tag--warning {
  background: color-mix(in srgb, var(--app-warning-color) 15%, var(--app-card-color));
  color: var(--app-warning-color);
}

.kx-tag--error {
  background: color-mix(in srgb, var(--app-error-color) 15%, var(--app-card-color));
  color: var(--app-error-color);
}

.kx-tag--info {
  background: color-mix(in srgb, var(--app-info-color) 15%, var(--app-card-color));
  color: var(--app-info-color);
}

a.kx-tag {
  text-decoration: none;
}

a.kx-tag:hover {
  text-decoration: underline;
}
</style>

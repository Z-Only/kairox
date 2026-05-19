<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    tone?:
      | "neutral"
      | "success"
      | "warning"
      | "error"
      | "info"
      | "muted"
      | "override"
      | "disabled-by"
      | "source-builtin"
      | "source-user"
      | "source-project"
      | "source-local";
    dataTest?: string;
  }>(),
  {
    tone: "neutral",
    dataTest: undefined
  }
);
</script>

<template>
  <span
    :class="['tag', 'settings-status-tag', `settings-status-tag--${props.tone}`]"
    :data-test="props.dataTest"
  >
    <slot />
  </span>
</template>

<style scoped>
.settings-status-tag {
  font-weight: 500;
}

.settings-status-tag--neutral {
  background: var(--app-hover-color);
  color: var(--app-text-color-2);
}

.settings-status-tag--muted,
.settings-status-tag--source-builtin {
  background: var(--app-muted-surface-color);
  color: var(--app-text-color-3);
}

.settings-status-tag--success {
  background: color-mix(in srgb, var(--app-success-color) 15%, transparent);
  color: var(--app-success-color);
}

.settings-status-tag--warning,
.settings-status-tag--override {
  background: color-mix(in srgb, var(--app-warning-color) 15%, transparent);
  color: var(--app-warning-color);
}

.settings-status-tag--error,
.settings-status-tag--disabled-by {
  background: color-mix(in srgb, var(--app-error-color) 15%, transparent);
  color: var(--app-error-color);
}

.settings-status-tag--info,
.settings-status-tag--source-user {
  background: color-mix(in srgb, var(--app-info-color) 15%, transparent);
  color: var(--app-info-color);
}

.settings-status-tag--source-project {
  background: color-mix(in srgb, var(--app-primary-color) 15%, transparent);
  color: var(--app-primary-color);
}

.settings-status-tag--source-local {
  background: color-mix(in srgb, var(--app-success-color) 15%, transparent);
  color: var(--app-success-color);
}
</style>

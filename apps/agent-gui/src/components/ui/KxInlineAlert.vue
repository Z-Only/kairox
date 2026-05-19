<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    tone?: "info" | "success" | "warning" | "error";
    dataTest?: string;
    compact?: boolean;
  }>(),
  {
    tone: "info",
    dataTest: undefined,
    compact: false
  }
);

const role = computed(() =>
  props.tone === "error" || props.tone === "warning" ? "alert" : "status"
);
</script>

<template>
  <div
    :class="[
      'kx-inline-alert',
      `kx-inline-alert--${tone}`,
      { 'kx-inline-alert--compact': compact }
    ]"
    :role="role"
    :data-test="dataTest"
  >
    <slot />
  </div>
</template>

<style scoped>
.kx-inline-alert {
  border-radius: 4px;
  padding: 8px 12px;
  font-size: 13px;
  line-height: 1.45;
}

.kx-inline-alert--compact {
  padding: 6px 10px;
}

.kx-inline-alert--info {
  background: var(--app-bg-color);
  color: var(--app-info-color);
}

.kx-inline-alert--success {
  background: var(--app-success-bg);
  color: var(--app-success-color);
}

.kx-inline-alert--warning {
  background: var(--app-warning-bg);
  color: var(--app-warning-color);
}

.kx-inline-alert--error {
  background: var(--app-error-bg);
  color: var(--app-error-color);
}
</style>

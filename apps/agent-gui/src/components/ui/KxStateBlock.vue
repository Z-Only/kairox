<script setup lang="ts">
type StateBlockTone = "empty" | "loading" | "info" | "success" | "warning" | "error";

const props = withDefaults(
  defineProps<{
    tone?: StateBlockTone;
    role?: string;
    dataTest?: string;
    compact?: boolean;
  }>(),
  {
    tone: "empty",
    role: undefined,
    dataTest: undefined,
    compact: false
  }
);

const resolvedRole = computed(() => {
  if (props.role !== undefined) return props.role;
  if (props.tone === "error") return "alert";
  if (props.tone === "loading" || props.tone === "success" || props.tone === "info") {
    return "status";
  }
  return undefined;
});
</script>

<template>
  <div
    :class="['kx-state-block', `kx-state-block--${tone}`, { 'kx-state-block--compact': compact }]"
    :role="resolvedRole"
    :data-test="dataTest"
  >
    <slot />
  </div>
</template>

<style scoped>
.kx-state-block {
  display: flex;
  box-sizing: border-box;
  min-width: 0;
  max-width: 100%;
  min-height: 44px;
  align-items: center;
  justify-content: center;
  gap: var(--app-space-2);
  flex-wrap: wrap;
  padding: var(--app-space-4);
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-lg);
  background: color-mix(in srgb, var(--app-panel-color) 70%, var(--app-card-color));
  color: var(--app-text-color-2);
  font-size: var(--app-text-base);
  line-height: 1.45;
  text-align: center;
  box-shadow: var(--app-shadow-sm);
}

.kx-state-block--compact {
  min-height: 0;
  justify-content: flex-start;
  padding: var(--app-space-3) var(--app-space-4);
  text-align: left;
}

.kx-state-block--empty {
  min-height: 96px;
  border-style: dashed;
  color: var(--app-text-color-3);
}

.kx-state-block--loading,
.kx-state-block--info {
  border-color: color-mix(in srgb, var(--app-info-color) 28%, var(--app-border-color));
  background: color-mix(in srgb, var(--app-info-color) 8%, var(--app-card-color));
  color: var(--app-text-color-2);
}

.kx-state-block--success {
  border-color: color-mix(in srgb, var(--app-success-color) 34%, var(--app-border-color));
  background: color-mix(in srgb, var(--app-success-color) 8%, var(--app-card-color));
}

.kx-state-block--warning {
  border-color: color-mix(in srgb, var(--app-warning-color) 34%, var(--app-border-color));
  background: color-mix(in srgb, var(--app-warning-color) 9%, var(--app-card-color));
}

.kx-state-block--error {
  border-color: color-mix(in srgb, var(--app-error-color) 38%, var(--app-border-color));
  background: color-mix(in srgb, var(--app-error-color) 9%, var(--app-card-color));
  color: var(--app-error-color);
}

@media (max-width: 640px) {
  .kx-state-block {
    padding: var(--app-space-3);
  }
}
</style>

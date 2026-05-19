<script setup lang="ts">
type ButtonVariant = "default" | "primary" | "danger" | "ghost";
type ButtonSize = "default" | "sm" | "xs";

const props = withDefaults(
  defineProps<{
    variant?: ButtonVariant;
    size?: ButtonSize;
    dataTest?: string;
    disabled?: boolean;
    type?: "button" | "submit" | "reset";
    ariaLabel?: string;
    title?: string;
  }>(),
  {
    variant: "default",
    size: "default",
    dataTest: undefined,
    disabled: false,
    type: "button",
    ariaLabel: undefined,
    title: undefined
  }
);
</script>

<template>
  <button
    :type="props.type"
    :class="['kx-button', `kx-button--${props.variant}`, `kx-button--size-${props.size}`]"
    :data-test="props.dataTest"
    :disabled="props.disabled"
    :aria-label="props.ariaLabel"
    :title="props.title"
  >
    <slot />
  </button>
</template>

<style scoped>
.kx-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-width: 0;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-md, 6px);
  background: var(--app-card-color);
  color: var(--app-text-color);
  cursor: pointer;
  font: inherit;
  line-height: 1.35;
  white-space: nowrap;
}

@media (prefers-reduced-motion: no-preference) {
  .kx-button {
    transition:
      background-color 0.15s,
      border-color 0.15s,
      box-shadow 0.15s,
      color 0.15s;
  }
}

.kx-button--default {
  background: var(--app-card-color);
}

.kx-button--default:hover:not(:disabled) {
  background: var(--app-hover-color);
}

.kx-button--primary {
  border-color: var(--app-primary-color);
  background: var(--app-primary-color);
  color: var(--app-primary-contrast-color, #fff);
}

.kx-button--primary:hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--app-primary-color) 85%, #000);
  background: color-mix(in srgb, var(--app-primary-color) 85%, #000);
  box-shadow: 0 2px 8px color-mix(in srgb, var(--app-primary-color) 24%, transparent);
}

.kx-button--danger {
  border-color: var(--app-error-color);
  background: var(--app-error-color);
  color: #fff;
}

.kx-button--danger:hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--app-error-color) 85%, #000);
  background: color-mix(in srgb, var(--app-error-color) 85%, #000);
  box-shadow: 0 2px 8px color-mix(in srgb, var(--app-error-color) 24%, transparent);
}

.kx-button--ghost {
  border-color: transparent;
  background: transparent;
}

.kx-button--ghost:hover:not(:disabled) {
  background: var(--app-hover-color);
}

.kx-button--size-default {
  min-height: 34px;
  padding: 6px 14px;
  font-size: var(--app-text-base);
}

.kx-button--size-sm {
  min-height: 28px;
  padding: 3px 8px;
  font-size: var(--app-text-sm);
}

.kx-button--size-xs {
  min-height: 24px;
  padding: 2px 7px;
  font-size: 12px;
}

.kx-button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}

.kx-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
  box-shadow: none;
}
</style>

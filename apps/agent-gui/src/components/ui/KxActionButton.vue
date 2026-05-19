<script setup lang="ts">
type ActionButtonVariant = "default" | "primary" | "danger";
type ActionButtonSize = "default" | "compact";

withDefaults(
  defineProps<{
    variant?: ActionButtonVariant;
    size?: ActionButtonSize;
    dataTest?: string;
    disabled?: boolean;
    type?: "button" | "submit" | "reset";
    ariaLabel?: string;
  }>(),
  {
    variant: "default",
    size: "compact",
    dataTest: undefined,
    disabled: false,
    type: "button",
    ariaLabel: undefined
  }
);
</script>

<template>
  <button
    :type="type"
    :class="['kx-action-button', `kx-action-button--${variant}`, `kx-action-button--${size}`]"
    :data-test="dataTest"
    :disabled="disabled"
    :aria-label="ariaLabel"
  >
    <slot />
  </button>
</template>

<style scoped>
.kx-action-button {
  flex: 0 0 auto;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-sm, 4px);
  background: var(--app-card-color);
  color: var(--app-text-color);
  cursor: pointer;
  font: inherit;
  white-space: nowrap;
}

.kx-action-button--compact {
  min-height: 24px;
  padding: 2px 7px;
  font-size: 12px;
}

.kx-action-button--default:hover:not(:disabled) {
  border-color: var(--app-primary-color);
  color: var(--app-primary-color);
}

.kx-action-button--primary {
  border-color: var(--app-primary-color);
  background: var(--app-primary-color);
  color: var(--app-primary-contrast, var(--app-inverse-text-color, #fff));
}

.kx-action-button--danger:hover:not(:disabled) {
  border-color: var(--app-error-color, #d03050);
  color: var(--app-error-color, #d03050);
}

.kx-action-button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}

.kx-action-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
</style>

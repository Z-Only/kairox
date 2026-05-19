<script setup lang="ts">
type IconButtonVariant = "ghost" | "default" | "danger";
type IconButtonSize = "default" | "sm";

const props = withDefaults(
  defineProps<{
    label: string;
    title?: string;
    disabled?: boolean;
    busy?: boolean;
    variant?: IconButtonVariant;
    size?: IconButtonSize;
    dataTest?: string;
  }>(),
  {
    title: undefined,
    disabled: false,
    busy: false,
    variant: "ghost",
    size: "default",
    dataTest: undefined
  }
);

const buttonTitle = computed(() => props.title ?? props.label);
const isDisabled = computed(() => props.disabled || props.busy);
</script>

<template>
  <button
    type="button"
    :class="['kx-icon-button', `kx-icon-button--${variant}`, `kx-icon-button--size-${size}`]"
    :aria-label="label"
    :aria-busy="busy ? 'true' : undefined"
    :data-test="dataTest"
    :disabled="isDisabled"
    :title="buttonTitle"
  >
    <slot />
  </button>
</template>

<style scoped>
.kx-icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid transparent;
  cursor: pointer;
  background: transparent;
  color: inherit;
  font: inherit;
  border-radius: 4px;
}

.kx-icon-button--default {
  border-color: var(--app-border-color);
  background: var(--app-card-color);
}

.kx-icon-button--ghost:hover:not(:disabled),
.kx-icon-button--default:hover:not(:disabled) {
  background: var(--app-hover-color);
}

.kx-icon-button:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.kx-icon-button--size-default {
  width: 28px;
  height: 28px;
}
.kx-icon-button--size-sm {
  width: 24px;
  height: 24px;
}
.kx-icon-button svg {
  display: block;
  width: 18px;
  height: 18px;
  fill: currentColor;
}
.kx-icon-button--danger:hover:not(:disabled) {
  background: color-mix(in srgb, var(--app-error-color) 10%, transparent);
}

.kx-icon-button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>

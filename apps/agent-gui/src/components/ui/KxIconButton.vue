<script setup lang="ts">
type IconButtonVariant = "ghost" | "default" | "danger";

const props = withDefaults(
  defineProps<{
    label: string;
    title?: string;
    disabled?: boolean;
    busy?: boolean;
    variant?: IconButtonVariant;
    dataTest?: string;
  }>(),
  {
    title: undefined,
    disabled: false,
    busy: false,
    variant: "ghost",
    dataTest: undefined
  }
);

const buttonTitle = computed(() => props.title ?? props.label);
const isDisabled = computed(() => props.disabled || props.busy);
</script>

<template>
  <button
    type="button"
    :class="['kx-icon-button', `kx-icon-button--${variant}`]"
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
  border: none;
  cursor: pointer;
  background: transparent;
  color: inherit;
  font: inherit;
  padding: 4px;
  border-radius: 4px;
}
.kx-icon-button:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.kx-icon-button svg {
  display: block;
  width: 18px;
  height: 18px;
  fill: currentColor;
}
.kx-icon-button--danger:hover {
  background: color-mix(in srgb, var(--app-error-color) 10%, transparent);
}
</style>

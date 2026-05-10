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

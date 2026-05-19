<script setup lang="ts">
type ChipButtonSize = "default" | "compact";

const props = withDefaults(
  defineProps<{
    selected?: boolean;
    disabled?: boolean;
    size?: ChipButtonSize;
    dataTest?: string;
    type?: "button" | "submit" | "reset";
  }>(),
  {
    selected: false,
    disabled: false,
    size: "default",
    dataTest: undefined,
    type: "button"
  }
);
</script>

<template>
  <button
    :type="props.type"
    :class="[
      'kx-chip-button',
      `kx-chip-button--size-${props.size}`,
      props.selected ? 'kx-chip-button--selected' : 'kx-chip-button--default'
    ]"
    :aria-pressed="props.selected ? 'true' : 'false'"
    :data-test="props.dataTest"
    :disabled="props.disabled"
  >
    <slot />
  </button>
</template>

<style scoped>
.kx-chip-button {
  display: inline-flex;
  min-width: 0;
  align-items: center;
  justify-content: center;
  gap: 4px;
  border: 1px solid var(--app-border-color);
  border-radius: 999px;
  background: var(--app-card-color);
  color: var(--app-text-color);
  cursor: pointer;
  font-family: inherit;
  font-weight: 500;
  line-height: 1.35;
  white-space: nowrap;
}

@media (prefers-reduced-motion: no-preference) {
  .kx-chip-button {
    transition:
      background-color 0.15s,
      border-color 0.15s,
      color 0.15s;
  }
}

.kx-chip-button--default:hover:not(:disabled) {
  background: var(--app-hover-color);
}

.kx-chip-button--selected {
  border-color: var(--app-primary-color);
  background: var(--app-primary-color);
  color: var(--app-primary-contrast-color, #fff);
}

.kx-chip-button--default {
  background: var(--app-card-color);
}

.kx-chip-button--size-compact {
  min-height: 24px;
  padding: 3px 10px;
  font-size: var(--app-text-sm);
}

.kx-chip-button--size-default {
  min-height: 28px;
  padding: 4px 12px;
  font-size: var(--app-text-base);
}

.kx-chip-button:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}

.kx-chip-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
</style>

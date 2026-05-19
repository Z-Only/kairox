<script setup lang="ts">
import KxButton from "./KxButton.vue";

type InlineActionVariant =
  | "default"
  | "primary"
  | "danger"
  | "danger-ghost"
  | "ghost"
  | "success"
  | "warning";

const props = withDefaults(
  defineProps<{
    variant?: InlineActionVariant;
    dataTest?: string;
    disabled?: boolean;
    type?: "button" | "submit" | "reset";
    ariaLabel?: string;
    title?: string;
  }>(),
  {
    variant: "default",
    dataTest: undefined,
    disabled: false,
    type: "button",
    ariaLabel: undefined,
    title: undefined
  }
);
</script>

<template>
  <KxButton
    :variant="props.variant"
    size="sm"
    :data-test="props.dataTest"
    :disabled="props.disabled"
    :type="props.type"
    :aria-label="props.ariaLabel"
    :title="props.title"
  >
    <slot />
  </KxButton>
</template>

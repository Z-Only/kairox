<script setup lang="ts">
defineOptions({ inheritAttrs: false });

type SelectModelValue = string | number | null;
type SelectSize = "default" | "compact";

const props = withDefaults(
  defineProps<{
    modelValue?: SelectModelValue;
    dataTest?: string;
    disabled?: boolean;
    required?: boolean;
    ariaLabel?: string;
    size?: SelectSize;
  }>(),
  {
    modelValue: "",
    dataTest: undefined,
    disabled: false,
    required: false,
    ariaLabel: undefined,
    size: "default"
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  change: [event: Event];
}>();

const attrs = useAttrs();

const forwardedAttrs = computed(() => {
  const { class: _class, "aria-label": _ariaLabel, ...rest } = attrs;
  return rest;
});

const selectClasses = computed(() => [attrs.class, "kx-select", `kx-select--${props.size}`]);
const selectValue = computed(() => props.modelValue ?? "");
const effectiveAriaLabel = computed(() => props.ariaLabel ?? (attrs["aria-label"] as string));

function onChange(event: Event): void {
  emit("update:modelValue", (event.target as HTMLSelectElement).value);
  emit("change", event);
}
</script>

<template>
  <select
    v-bind="forwardedAttrs"
    :class="selectClasses"
    :value="selectValue"
    :data-test="dataTest"
    :disabled="disabled"
    :required="required"
    :aria-label="effectiveAriaLabel"
    @change="onChange"
  >
    <slot />
  </select>
</template>

<style scoped>
.kx-select {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  min-height: 34px;
  padding: 6px 10px;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-md, 6px);
  background: var(--app-elevated-color, var(--app-card-color));
  color: var(--app-text-color);
  font: inherit;
  font-size: var(--app-text-sm, 0.84rem);
  outline: none;
}

.kx-select--compact {
  min-height: 32px;
  padding: 4px 10px;
}

.kx-select:focus {
  border-color: var(--app-primary-color);
  box-shadow: var(--app-focus-ring);
}

.kx-select:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
  box-shadow: var(--app-focus-ring);
}

.kx-select:disabled {
  opacity: 0.68;
}
</style>

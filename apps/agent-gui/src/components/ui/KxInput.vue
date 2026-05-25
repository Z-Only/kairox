<script setup lang="ts">
defineOptions({ inheritAttrs: false });

type InputModelValue = string | number | null;
type InputSize = "default" | "compact";

const props = withDefaults(
  defineProps<{
    modelValue?: InputModelValue;
    modelModifiers?: { number?: boolean };
    dataTest?: string;
    type?: string;
    placeholder?: string;
    readonly?: boolean;
    disabled?: boolean;
    required?: boolean;
    ariaLabel?: string;
    size?: InputSize;
  }>(),
  {
    modelValue: "",
    modelModifiers: undefined,
    dataTest: undefined,
    type: "text",
    placeholder: undefined,
    readonly: false,
    disabled: false,
    required: false,
    ariaLabel: undefined,
    size: "default"
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: string | number];
  input: [event: Event];
}>();

const attrs = useAttrs();

const forwardedAttrs = computed(() => {
  const { class: _class, "aria-label": _ariaLabel, ...rest } = attrs;
  return rest;
});

const inputClasses = computed(() => [attrs.class, "kx-input", `kx-input--${props.size}`]);
const inputValue = computed(() => props.modelValue ?? "");
const effectiveAriaLabel = computed(() => props.ariaLabel ?? (attrs["aria-label"] as string));

function normalizeValue(value: string): string | number {
  if (!props.modelModifiers?.number) return value;
  const parsed = Number.parseFloat(value);
  return Number.isNaN(parsed) ? value : parsed;
}

function onInput(event: Event): void {
  emit("update:modelValue", normalizeValue((event.target as HTMLInputElement).value));
  emit("input", event);
}
</script>

<template>
  <input
    v-bind="forwardedAttrs"
    :class="inputClasses"
    :value="inputValue"
    :type="type"
    :data-test="dataTest"
    :placeholder="placeholder"
    :readonly="readonly"
    :disabled="disabled"
    :required="required"
    :aria-label="effectiveAriaLabel"
    autocapitalize="off"
    autocomplete="off"
    autocorrect="off"
    spellcheck="false"
    @input="onInput"
  />
</template>

<style scoped>
.kx-input {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  min-height: 34px;
  padding: 6px 10px;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-md, 6px);
  background: var(--app-card-color);
  color: var(--app-text-color);
  font: inherit;
  font-size: var(--app-text-sm, 0.84rem);
  outline: none;
}

.kx-input--compact {
  min-height: 32px;
  padding: 4px 10px;
}

.kx-input[type="search"] {
  flex: 1 1 auto;
  min-width: min(220px, 100%);
  max-width: 360px;
}

.kx-input:focus {
  border-color: var(--app-primary-color);
}

.kx-input:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}

.kx-input:disabled,
.kx-input[readonly] {
  opacity: 0.68;
}
</style>

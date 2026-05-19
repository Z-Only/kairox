<script setup lang="ts">
defineOptions({ inheritAttrs: false });

type TextareaVariant = "default" | "mono" | "preview" | "composer";
type TextareaResize = "vertical" | "none" | "both";

const props = withDefaults(
  defineProps<{
    modelValue?: string;
    dataTest?: string;
    placeholder?: string;
    rows?: number | string;
    readonly?: boolean;
    disabled?: boolean;
    ariaLabel?: string;
    variant?: TextareaVariant;
    resize?: TextareaResize;
  }>(),
  {
    modelValue: "",
    dataTest: undefined,
    placeholder: undefined,
    rows: 3,
    readonly: false,
    disabled: false,
    ariaLabel: undefined,
    variant: "default",
    resize: "vertical"
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  input: [event: Event];
}>();

const attrs = useAttrs();

const forwardedAttrs = computed(() => {
  const { class: _class, "aria-label": _ariaLabel, ...rest } = attrs;
  return rest;
});

const textareaClasses = computed(() => [
  attrs.class,
  "kx-textarea",
  `kx-textarea--${props.variant}`,
  `kx-textarea--resize-${props.resize}`
]);

const effectiveAriaLabel = computed(() => props.ariaLabel ?? (attrs["aria-label"] as string));

function onInput(event: Event): void {
  emit("update:modelValue", (event.target as HTMLTextAreaElement).value);
  emit("input", event);
}
</script>

<template>
  <textarea
    v-bind="forwardedAttrs"
    :class="textareaClasses"
    :value="modelValue"
    :data-test="dataTest"
    :placeholder="placeholder"
    :rows="rows"
    :readonly="readonly"
    :disabled="disabled"
    :aria-label="effectiveAriaLabel"
    @input="onInput"
  />
</template>

<style scoped>
.kx-textarea {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  min-height: 34px;
  padding: 8px 10px;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-md, 6px);
  background: var(--app-card-color);
  color: var(--app-text-color);
  font: inherit;
  font-size: 0.84rem;
  line-height: 1.45;
  outline: none;
}

.kx-textarea:focus {
  border-color: var(--app-primary-color);
}

.kx-textarea:focus-visible {
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--app-primary-color) 24%, transparent);
}

.kx-textarea:read-only {
  background: color-mix(in srgb, var(--app-card-color) 64%, transparent);
  color: var(--app-text-color-2);
  cursor: default;
}

.kx-textarea:disabled {
  background: color-mix(in srgb, var(--app-card-color) 48%, transparent);
  color: var(--app-text-color-2);
  cursor: not-allowed;
  opacity: 0.68;
}

.kx-textarea--mono,
.kx-textarea--preview {
  font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
}

.kx-textarea--preview {
  border-color: var(--app-primary-color);
}

.kx-textarea--composer {
  flex: 1 1 auto;
  padding: 6px 10px;
  font-size: 13px;
  font-family: inherit;
}

.kx-textarea--resize-vertical {
  resize: vertical;
}

.kx-textarea--resize-none {
  resize: none;
}

.kx-textarea--resize-both {
  resize: both;
}
</style>

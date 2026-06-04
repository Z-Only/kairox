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
    autoResize?: boolean;
    maxAutoResizeHeight?: number | string;
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
    resize: "vertical",
    autoResize: false,
    maxAutoResizeHeight: undefined
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  input: [event: Event];
  change: [event: Event];
}>();

const attrs = useAttrs();
const textareaRef = ref<HTMLTextAreaElement | null>(null);

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

const maxAutoResizeHeightPx = computed(() => {
  if (!props.maxAutoResizeHeight) return null;
  const parsed = Number(props.maxAutoResizeHeight);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
});

function resetAutoResizeStyles(textarea: HTMLTextAreaElement): void {
  textarea.style.height = "";
  textarea.style.overflowY = "";
}

function resizeToContent(): void {
  const textarea = textareaRef.value;
  if (!textarea) return;

  if (!props.autoResize) {
    resetAutoResizeStyles(textarea);
    return;
  }

  textarea.style.height = "auto";
  const maxHeight = maxAutoResizeHeightPx.value;
  const scrollHeight = textarea.scrollHeight;
  const nextHeight = maxHeight ? Math.min(scrollHeight, maxHeight) : scrollHeight;
  textarea.style.height = `${nextHeight}px`;
  textarea.style.overflowY = maxHeight && scrollHeight > maxHeight ? "auto" : "hidden";
}

function emitModelValue(event: Event): void {
  emit("update:modelValue", (event.target as HTMLTextAreaElement).value);
}

function onInput(event: Event): void {
  resizeToContent();
  emitModelValue(event);
  emit("input", event);
}

function onChange(event: Event): void {
  resizeToContent();
  emitModelValue(event);
  emit("change", event);
}

watch(
  () => [props.modelValue, props.autoResize, props.maxAutoResizeHeight, props.rows],
  () => {
    void nextTick(resizeToContent);
  },
  { immediate: true }
);

onMounted(resizeToContent);
</script>

<template>
  <textarea
    ref="textareaRef"
    v-bind="forwardedAttrs"
    :class="textareaClasses"
    :value="modelValue"
    :data-test="dataTest"
    :placeholder="placeholder"
    :rows="rows"
    :readonly="readonly"
    :disabled="disabled"
    :aria-label="effectiveAriaLabel"
    autocapitalize="off"
    autocomplete="off"
    autocorrect="off"
    spellcheck="false"
    @input="onInput"
    @change="onChange"
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
  background: var(--app-elevated-color, var(--app-card-color));
  color: var(--app-text-color);
  font: inherit;
  font-size: 0.84rem;
  line-height: 1.45;
  outline: none;
}

.kx-textarea:focus {
  border-color: var(--app-primary-color);
  box-shadow: var(--app-focus-ring);
}

.kx-textarea:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
  box-shadow: var(--app-focus-ring);
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

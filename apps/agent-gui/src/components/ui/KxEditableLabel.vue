<script setup lang="ts">
const modelValue = defineModel<string>({ required: true });

withDefaults(
  defineProps<{
    inputDataTest?: string;
    confirmDataTest?: string;
    confirmLabel: string;
    inputRef?: (el: Element | null) => void;
  }>(),
  {
    inputDataTest: undefined,
    confirmDataTest: undefined,
    inputRef: undefined
  }
);

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();
</script>

<template>
  <span class="kx-editable-label">
    <input
      :ref="inputRef"
      v-model="modelValue"
      class="kx-editable-label__input rename-input"
      :data-test="inputDataTest"
      autocapitalize="off"
      autocomplete="off"
      autocorrect="off"
      spellcheck="false"
      @keydown.enter="emit('confirm')"
      @keydown.escape="emit('cancel')"
      @blur="emit('confirm')"
      @click.stop
    />
    <KxTooltip :text="confirmLabel">
      <KxIconButton
        :label="confirmLabel"
        :title="confirmLabel"
        :data-test="confirmDataTest"
        @mousedown.prevent
        @click.stop="emit('confirm')"
      >
        <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
          <path d="m8.25 13.25-3-3L6.3 9.2l1.95 1.94 5.45-5.44 1.05 1.05-6.5 6.5Z" />
        </svg>
      </KxIconButton>
    </KxTooltip>
  </span>
</template>

<style scoped>
.kx-editable-label {
  display: inline-flex;
  min-width: 0;
  flex: 1;
  align-items: center;
  gap: 4px;
}

.kx-editable-label__input {
  box-sizing: border-box;
  flex: 1;
  min-width: 0;
  min-height: 28px;
  padding: 2px 6px;
  border: 1px solid var(--app-primary-color);
  border-radius: var(--app-radius-sm, 4px);
  background: var(--app-card-color);
  color: var(--app-text-color);
  font: inherit;
  font-size: 13px;
  outline: none;
}

.kx-editable-label__input:focus-visible {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}
</style>

<script setup lang="ts">
import type { StyleValue } from "vue";

defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    open: boolean;
    title?: string;
    description?: string;
    closeLabel?: string;
    width?: string;
    bodyDataTest?: string;
    closeOnBackdrop?: boolean;
    showClose?: boolean;
  }>(),
  {
    closeLabel: "Close",
    width: "520px",
    closeOnBackdrop: true,
    showClose: true
  }
);

const emit = defineEmits<{
  close: [];
}>();

const dialogRef = ref<HTMLDialogElement | null>(null);

const panelStyle = computed<StyleValue>(() => ({
  "--kx-modal-width": props.width
}));

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      void nextTick(() => {
        const dialog = dialogRef.value;
        if (dialog && !dialog.open) dialog.showModal?.();
        dialog?.setAttribute("open", "");
      });
    } else {
      dialogRef.value?.close?.();
    }
  },
  { immediate: true }
);

function requestClose(): void {
  emit("close");
}

function onBackdropClick(event: MouseEvent): void {
  if (props.closeOnBackdrop && event.target === dialogRef.value) {
    requestClose();
  }
}
</script>

<template>
  <dialog
    v-if="open"
    ref="dialogRef"
    v-bind="$attrs"
    class="kx-modal"
    @click="onBackdropClick"
    @cancel.prevent="requestClose"
  >
    <div class="kx-modal__panel" :style="panelStyle">
      <header v-if="title || description || $slots.header || showClose" class="kx-modal__header">
        <div v-if="title || description" class="kx-modal__header-text">
          <h3 v-if="title" class="kx-modal__title">{{ title }}</h3>
          <p v-if="description" class="kx-modal__description">{{ description }}</p>
        </div>
        <slot v-else name="header" />
        <button
          v-if="showClose"
          class="btn kx-modal__close"
          type="button"
          :aria-label="closeLabel"
          @click="requestClose"
        >
          &times;
        </button>
      </header>

      <div class="kx-modal__body" :data-test="bodyDataTest">
        <slot />
      </div>

      <footer v-if="$slots.footer" class="kx-modal__footer">
        <slot name="footer" />
      </footer>
    </div>
  </dialog>
</template>

<style scoped>
.kx-modal {
  position: fixed;
  inset: 0;
  display: grid;
  width: 100vw;
  max-width: none;
  height: 100dvh;
  max-height: none;
  place-items: center;
  padding: 0;
  margin: 0;
  border: 0;
  background: transparent;
  color: var(--app-text-color);
  overflow: auto;
}

.kx-modal::backdrop {
  background: var(--app-backdrop-color);
}

.kx-modal__panel {
  --kx-modal-width: 520px;
  width: min(var(--kx-modal-width), calc(100vw - 48px));
  max-height: min(85vh, calc(100vh - 48px));
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-md);
  background: var(--app-card-color, var(--app-bg-color));
  box-shadow: var(--app-overlay-shadow, var(--app-shadow-2));
}

.kx-modal__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 18px;
  border-bottom: 1px solid var(--app-border-color);
}

.kx-modal__header-text {
  min-width: 0;
}

.kx-modal__title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--app-text-color);
}

.kx-modal__description {
  margin: 4px 0 0;
  font-size: 13px;
  line-height: 1.4;
  color: var(--app-text-color-2);
}

.kx-modal__close {
  flex: 0 0 auto;
  padding: 2px 7px;
  font-size: 18px;
  line-height: 1;
}

.kx-modal__body {
  min-height: 0;
  overflow: auto;
  padding: 16px 18px;
}

.kx-modal__footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 18px;
  border-top: 1px solid var(--app-border-color);
}
</style>

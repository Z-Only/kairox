<script setup lang="ts">
const props = defineProps<{
  open: boolean;
  title?: string;
  description?: string;
}>();

const emit = defineEmits<{
  close: [];
}>();

const dialogRef = ref<HTMLDialogElement | null>(null);

watch(
  () => props.open,
  (val) => {
    if (val) {
      nextTick(() => dialogRef.value?.showModal?.());
    } else {
      dialogRef.value?.close?.();
    }
  }
);

function onClose() {
  emit("close");
}

function onBackdropClick(event: MouseEvent) {
  if (event.target === dialogRef.value) {
    emit("close");
  }
}
</script>

<template>
  <dialog
    v-if="open"
    ref="dialogRef"
    class="modal-dialog"
    @click="onBackdropClick"
    @close="onClose"
    @cancel.prevent="onClose"
  >
    <div class="modal-dialog__inner">
      <header v-if="title || $slots.header" class="modal-dialog__header">
        <div v-if="title" class="modal-dialog__header-text">
          <h3>{{ title }}</h3>
          <p v-if="description">{{ description }}</p>
        </div>
        <slot v-else name="header" />
      </header>

      <div class="modal-dialog__body">
        <slot />
      </div>

      <footer v-if="$slots.footer" class="modal-dialog__footer">
        <slot name="footer" />
      </footer>
    </div>
  </dialog>
</template>

<style scoped>
.modal-dialog {
  border: none;
  border-radius: 12px;
  padding: 0;
  max-width: 520px;
  width: calc(100vw - 48px);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
  background: var(--app-card-color, var(--app-bg-color));
  color: var(--app-text-color);
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  margin: 0;
  max-height: 85vh;
  overflow-y: auto;
}

.modal-dialog::backdrop {
  background: rgba(0, 0, 0, 0.4);
}

.modal-dialog__inner {
  display: flex;
  flex-direction: column;
}

.modal-dialog__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 20px 20px 0;
}

.modal-dialog__header-text h3 {
  margin: 0 0 4px;
  font-size: 16px;
  font-weight: 600;
}

.modal-dialog__header-text p {
  margin: 0;
  font-size: 13px;
  color: var(--app-text-color-2);
}

.modal-dialog__body {
  padding: 16px 20px;
}

.modal-dialog__footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 0 20px 20px;
}
</style>

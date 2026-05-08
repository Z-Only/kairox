<script setup lang="ts">
import { provide, ref } from "vue";
import { confirmDialogKey, type ConfirmOptions, type ConfirmAPI } from "@/composables/useConfirm";

const dialogRef = ref<HTMLDialogElement | null>(null);
const currentOptions = ref<ConfirmOptions>({
  message: "",
  title: "",
  confirmText: "Confirm",
  cancelText: "Cancel",
  type: "info"
});

let resolvePromise: ((value: boolean) => void) | null = null;

function confirm(options: ConfirmOptions): Promise<boolean> {
  currentOptions.value = {
    title: options.title ?? "",
    message: options.message,
    confirmText: options.confirmText ?? "Confirm",
    cancelText: options.cancelText ?? "Cancel",
    type: options.type ?? "info"
  };
  dialogRef.value?.showModal();
  return new Promise<boolean>((resolve) => {
    resolvePromise = resolve;
  });
}

function handleConfirm() {
  dialogRef.value?.close();
  resolvePromise?.(true);
  resolvePromise = null;
}

function handleCancel() {
  dialogRef.value?.close();
  resolvePromise?.(false);
  resolvePromise = null;
}

const api: ConfirmAPI = { confirm };
provide(confirmDialogKey, api);
</script>

<template>
  <dialog ref="dialogRef" class="confirm-dialog" @cancel="handleCancel">
    <div v-if="currentOptions.title" class="confirm-dialog__header">
      {{ currentOptions.title }}
    </div>
    <div class="confirm-dialog__body">
      {{ currentOptions.message }}
    </div>
    <div class="confirm-dialog__footer">
      <button class="btn" data-test="confirm-cancel" @click="handleCancel">
        {{ currentOptions.cancelText }}
      </button>
      <button
        :class="['btn', currentOptions.type === 'error' ? 'btn-danger' : 'btn-primary']"
        data-test="confirm-ok"
        @click="handleConfirm"
      >
        {{ currentOptions.confirmText }}
      </button>
    </div>
  </dialog>
  <slot />
</template>

<style scoped>
.confirm-dialog__header {
  padding: 16px 20px 0;
  font-weight: 600;
  font-size: 15px;
}

.confirm-dialog__body {
  padding: 16px 20px;
  font-size: 14px;
  line-height: 1.5;
  color: var(--app-text-color-2);
}

.confirm-dialog__footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 0 20px 16px;
}
</style>

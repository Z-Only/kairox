<script setup lang="ts">
import { provide, ref } from "vue";
import { confirmDialogKey, type ConfirmOptions, type ConfirmAPI } from "@/composables/useConfirm";

const open = ref(false);
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
  open.value = true;
  return new Promise<boolean>((resolve) => {
    resolvePromise = resolve;
  });
}

function handleConfirm() {
  open.value = false;
  resolvePromise?.(true);
  resolvePromise = null;
}

function handleCancel() {
  open.value = false;
  resolvePromise?.(false);
  resolvePromise = null;
}

const api: ConfirmAPI = { confirm };
provide(confirmDialogKey, api);
</script>

<template>
  <KxModal
    :open="open"
    :title="currentOptions.title"
    :close-label="currentOptions.cancelText"
    width="420px"
    data-test="confirm-dialog"
    @close="handleCancel"
  >
    <div class="confirm-dialog__body">
      {{ currentOptions.message }}
    </div>

    <template #footer>
      <KxButton data-test="confirm-cancel" @click="handleCancel">
        {{ currentOptions.cancelText }}
      </KxButton>
      <KxButton
        :variant="currentOptions.type === 'error' ? 'danger' : 'primary'"
        data-test="confirm-ok"
        @click="handleConfirm"
      >
        {{ currentOptions.confirmText }}
      </KxButton>
    </template>
  </KxModal>
  <slot />
</template>

<style scoped>
.confirm-dialog__body {
  font-size: 14px;
  line-height: 1.5;
  color: var(--app-text-color-2);
}
</style>

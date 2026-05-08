import { inject, type InjectionKey } from "vue";

export interface ConfirmOptions {
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  type?: "info" | "warning" | "error";
}

export interface ConfirmAPI {
  confirm: (options: ConfirmOptions) => Promise<boolean>;
}

export const confirmDialogKey: InjectionKey<ConfirmAPI> = Symbol("confirmDialog");

export function useConfirm(): ConfirmAPI {
  const api = inject(confirmDialogKey);
  if (!api) {
    throw new Error("useConfirm() requires <ConfirmDialog /> to be mounted in a parent component");
  }
  return api;
}

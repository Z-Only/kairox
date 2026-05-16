import { nextTick, ref, type Ref } from "vue";

type RenameConfirmHandler = (id: string, title: string) => Promise<void> | void;

export interface SidebarRenameController {
  editingId: Ref<string | null>;
  title: Ref<string>;
  input: Ref<HTMLInputElement | null>;
  start: (id: string, currentTitle: string) => void;
  bindInput: (el: Element | null, itemId: string) => void;
  confirm: () => Promise<void>;
  cancel: () => void;
}

export function useSidebarRename(options: {
  onConfirm: RenameConfirmHandler;
  onStart?: () => void;
}): SidebarRenameController {
  const editingId = ref<string | null>(null);
  const title = ref("");
  const input = ref<HTMLInputElement | null>(null);

  function start(id: string, currentTitle: string) {
    options.onStart?.();
    editingId.value = id;
    title.value = currentTitle;
    nextTick(() => {
      input.value?.focus();
      input.value?.select();
    });
  }

  function bindInput(el: Element | null, itemId: string) {
    if (editingId.value === itemId) {
      input.value = (el as HTMLInputElement) ?? null;
    }
  }

  async function confirm() {
    if (editingId.value && title.value.trim()) {
      await options.onConfirm(editingId.value, title.value.trim());
    }
    editingId.value = null;
  }

  function cancel() {
    editingId.value = null;
  }

  return {
    editingId,
    title,
    input,
    start,
    bindInput,
    confirm,
    cancel
  };
}

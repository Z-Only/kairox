import { computed, getCurrentScope, onScopeDispose, ref, watch, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { CommandDef } from "@/composables/useCommandRegistry";
import { useDraftStore } from "@/composables/useDraftStore";

export interface Attachment {
  id: string;
  path: string;
  name: string;
  mimeType: string;
}

export interface DraftStore {
  loadDraft(sessionId: string): Promise<string>;
  saveDraft(sessionId: string, text: string): Promise<void>;
  clearDraft(sessionId: string): Promise<void>;
}

export interface ChatComposerSession {
  currentSessionId: string | null;
  currentProfile: string;
  isStreaming: boolean;
  reportSendError?: (message: string) => void;
}

interface UseChatComposerOptions {
  session: ChatComposerSession;
  draftStore?: DraftStore;
  invokeFn?: typeof invoke;
  openFileDialog?: typeof open;
  notify?: (type: "error", message: string) => void;
  t?: (key: string, values?: Record<string, unknown>) => string;
}

function createAttachmentId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
}

export function mimeFromExtension(ext: string): string {
  const map: Record<string, string> = {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    gif: "image/gif",
    webp: "image/webp",
    svg: "image/svg+xml",
    bmp: "image/bmp",
    pdf: "application/pdf",
    txt: "text/plain",
    md: "text/markdown",
    rs: "text/x-rust",
    py: "text/x-python",
    ts: "text/typescript",
    js: "text/javascript",
    json: "application/json",
    yaml: "application/x-yaml",
    yml: "application/x-yaml",
    toml: "application/toml",
    html: "text/html",
    css: "text/css",
    csv: "text/csv",
    xml: "application/xml",
    sh: "application/x-sh",
    bash: "application/x-sh",
    zsh: "application/x-sh",
    log: "text/plain"
  };
  return map[ext] || "application/octet-stream";
}

export function useChatComposer(options: UseChatComposerOptions) {
  const session = options.session;
  const draftStore = options.draftStore ?? useDraftStore();
  const invokeFn = options.invokeFn ?? invoke;
  const openFileDialog = options.openFileDialog ?? open;
  const notify = options.notify ?? (() => undefined);
  const t = options.t ?? ((key) => key);

  const inputText = ref("");
  const showCommandPalette = ref(false);
  const showMentionPalette = ref(false);
  const paletteFilter = ref("");
  const attachments = ref<Attachment[]>([]);
  const switchingModel = ref(false);
  let draftTimer: ReturnType<typeof setTimeout> | null = null;

  function clearDraftTimer() {
    if (draftTimer) {
      clearTimeout(draftTimer);
      draftTimer = null;
    }
  }

  watch(
    () => session.currentSessionId,
    async (newId, oldId) => {
      if (oldId && inputText.value.trim()) {
        await draftStore.saveDraft(oldId, inputText.value);
      }
      if (newId) {
        inputText.value = await draftStore.loadDraft(newId);
      } else {
        inputText.value = "";
      }
    }
  );

  watch(inputText, (val) => {
    clearDraftTimer();
    draftTimer = setTimeout(async () => {
      if (session.currentSessionId) {
        await draftStore.saveDraft(session.currentSessionId, val);
      }
    }, 500);
  });

  if (getCurrentScope()) {
    onScopeDispose(clearDraftTimer);
  }

  const sendDisabled = computed(
    () => session.isStreaming || (!inputText.value.trim() && attachments.value.length === 0)
  );

  function updatePaletteForCursor(text: string, cursorPos: number) {
    const textBeforeCursor = text.slice(0, cursorPos);
    const slashMatch = textBeforeCursor.match(/^\s*\/([^\s/]*)$/);
    const atMatch = textBeforeCursor.match(/(?:^|\s)@([^\s@]*)$/);

    if (slashMatch) {
      paletteFilter.value = slashMatch[1] || "";
      showCommandPalette.value = true;
      showMentionPalette.value = false;
    } else if (atMatch) {
      paletteFilter.value = atMatch[1] || "";
      showMentionPalette.value = true;
      showCommandPalette.value = false;
    } else {
      closePalettes();
    }
  }

  function handleInput(e: Event) {
    const textarea = e.target as HTMLTextAreaElement;
    updatePaletteForCursor(textarea.value, textarea.selectionStart);
  }

  function closePalettes() {
    showCommandPalette.value = false;
    showMentionPalette.value = false;
  }

  function onSelectCommand(cmd: CommandDef) {
    if (cmd.insertText) {
      const cursorPos = inputText.value.length;
      const textBeforeCursor = inputText.value.slice(0, cursorPos);
      const match = textBeforeCursor.match(/^\s*\/[^\s]*$/);
      if (match) {
        const before = inputText.value.slice(0, match.index !== undefined ? match.index : 0);
        const after = inputText.value.slice(cursorPos);
        inputText.value = before + cmd.insertText + after;
      }
    }
    closePalettes();
  }

  function onSelectSkill(skillId: string) {
    const cursorPos = inputText.value.length;
    const textBeforeCursor = inputText.value.slice(0, cursorPos);
    const match = textBeforeCursor.match(/^\s*\/[^\s]*$/);
    if (match) {
      const before = inputText.value.slice(0, match.index !== undefined ? match.index : 0);
      const after = inputText.value.slice(cursorPos);
      inputText.value = before + `/skills ${skillId} ` + after;
    }
    closePalettes();
  }

  function onSelectFile(path: string) {
    const cursorPos = inputText.value.length;
    const textBeforeCursor = inputText.value.slice(0, cursorPos);
    const match = textBeforeCursor.match(/(?:^|\s)@[^\s]*$/);
    if (match) {
      const before = inputText.value.slice(0, match.index !== undefined ? match.index : 0);
      const after = inputText.value.slice(cursorPos);
      inputText.value = before + `@${path} ` + after;
    }
    closePalettes();
  }

  function addFilePaths(paths: string[]) {
    for (const filePath of paths) {
      if (!filePath) continue;
      const name = filePath.split(/[\\/]/).pop() || filePath;
      const ext = name.split(".").pop()?.toLowerCase() || "";
      attachments.value = [
        ...attachments.value,
        {
          id: createAttachmentId(),
          path: filePath,
          name,
          mimeType: mimeFromExtension(ext)
        }
      ];
    }
  }

  async function pickFiles() {
    try {
      const selected = await openFileDialog({ multiple: true });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      addFilePaths(paths.map((path) => String(path)));
    } catch (e) {
      console.error("File picker error:", e);
    }
  }

  function removeAttachment(id: string) {
    attachments.value = attachments.value.filter((a) => a.id !== id);
  }

  async function sendMessage() {
    const content = inputText.value.trim();
    if ((!content && attachments.value.length === 0) || session.isStreaming) return;

    const payload = {
      content,
      attachments: attachments.value.map((a) => ({
        path: a.path,
        name: a.name,
        mime_type: a.mimeType
      }))
    };

    inputText.value = "";
    attachments.value = [];
    if (session.currentSessionId) {
      draftStore.clearDraft(session.currentSessionId);
    }
    try {
      await invokeFn("send_message", payload);
    } catch (e) {
      console.error("Failed to send message:", e);
      session.reportSendError?.(String(e));
      notify("error", t("chat.sendFailed", { error: String(e) }));
    }
  }

  async function cancelSession() {
    try {
      await invokeFn("cancel_session");
    } catch (e) {
      console.error("Failed to cancel session:", e);
      notify("error", t("chat.cancelFailed", { error: String(e) }));
    }
  }

  async function selectModelProfile(alias: string, modelPopoverOpen: Ref<boolean>) {
    if (switchingModel.value) return;
    if (alias === session.currentProfile) {
      modelPopoverOpen.value = false;
      return;
    }
    if (!session.currentSessionId) return;

    switchingModel.value = true;
    try {
      await invokeFn("switch_model", {
        sessionId: session.currentSessionId,
        profileAlias: alias
      });
      session.currentProfile = alias;
      modelPopoverOpen.value = false;
    } catch (e) {
      console.error("Failed to switch model:", e);
      const errMsg = String(e);
      if (errMsg.includes("unknown model")) {
        notify("error", t("errors.modelNotFound", { alias }));
      } else {
        notify("error", t("context.switchModelFailed", { error: errMsg }));
      }
    } finally {
      switchingModel.value = false;
    }
  }

  return {
    inputText,
    showCommandPalette,
    showMentionPalette,
    paletteFilter,
    attachments,
    switchingModel,
    sendDisabled,
    updatePaletteForCursor,
    handleInput,
    closePalettes,
    onSelectCommand,
    onSelectSkill,
    onSelectFile,
    addFilePaths,
    pickFiles,
    removeAttachment,
    sendMessage,
    cancelSession,
    selectModelProfile
  };
}

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

export interface QueuedMessage {
  id: string;
  content: string;
  attachments: Attachment[];
}

export interface DraftStore {
  loadDraft(sessionId: string): Promise<string>;
  saveDraft(sessionId: string, text: string): Promise<void>;
  clearDraft(sessionId: string): Promise<void>;
}

export interface ChatComposerSession {
  currentSessionId: string | null;
  composerDraftKey?: string | null;
  currentProfile: string;
  currentReasoningEffort?: string | null;
  isStreaming: boolean;
  compacting?: boolean;
  reportSendError?: (message: string) => void;
  setPendingModelSelection?: (profile: string, reasoningEffort: string | null) => void;
  ensureSessionForSend?: () => Promise<void>;
  refreshCurrentSessionMetadata?: (firstMessageContent?: string) => Promise<void>;
  updateSessionProfile?: (sessionId: string, profile: string) => void;
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
  const queuedMessages = ref<QueuedMessage[]>([]);
  const sendingQueuedId = ref<string | null>(null);
  const switchingModel = ref(false);
  let draftTimer: ReturnType<typeof setTimeout> | null = null;
  let skipNextDraftLoad = false;
  let skipNextDraftSaveKey: string | null = null;

  function clearDraftTimer() {
    if (draftTimer) {
      clearTimeout(draftTimer);
      draftTimer = null;
    }
  }

  function currentDraftKey(): string | null {
    return session.composerDraftKey ?? session.currentSessionId;
  }

  watch(
    currentDraftKey,
    async (newId, oldId) => {
      if (oldId !== undefined) {
        queuedMessages.value = [];
        sendingQueuedId.value = null;
      }
      const inputBeforeLoad = inputText.value;
      const skipOldDraftSave = Boolean(oldId && oldId === skipNextDraftSaveKey);
      if (skipOldDraftSave) {
        skipNextDraftSaveKey = null;
      }
      if (oldId && inputText.value.trim() && !skipOldDraftSave) {
        await draftStore.saveDraft(oldId, inputText.value);
      }
      if (newId) {
        const skipLoadedDraft = skipNextDraftLoad;
        skipNextDraftLoad = false;
        const loadedDraft = await draftStore.loadDraft(newId);
        if (
          !skipLoadedDraft &&
          currentDraftKey() === newId &&
          inputText.value === inputBeforeLoad
        ) {
          inputText.value = loadedDraft;
        }
      } else if (inputText.value === inputBeforeLoad) {
        inputText.value = "";
      }
    },
    { immediate: true }
  );

  watch(inputText, (val) => {
    clearDraftTimer();
    draftTimer = setTimeout(async () => {
      const draftKey = currentDraftKey();
      if (draftKey) {
        await draftStore.saveDraft(draftKey, val);
      }
    }, 500);
  });

  if (getCurrentScope()) {
    onScopeDispose(clearDraftTimer);
  }

  const isQueueing = computed(() => session.isStreaming || Boolean(session.compacting));
  const sendDisabled = computed(() => !inputText.value.trim() && attachments.value.length === 0);

  watch(
    () => [session.isStreaming, Boolean(session.compacting)] as const,
    ([isStreaming, compacting]) => {
      if (!isStreaming && !compacting) {
        void sendNextQueuedMessage();
      }
    },
    { flush: "sync" }
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

  function onSelectFile(path: string, workspacePath?: string) {
    const cursorPos = inputText.value.length;
    const textBeforeCursor = inputText.value.slice(0, cursorPos);
    const match = textBeforeCursor.match(/(?:^|\s)@[^\s]*$/);
    if (match) {
      const matchIndex = match.index !== undefined ? match.index : 0;
      const triggerText = match[0] ?? "";
      const leadingWhitespace =
        triggerText.length > 0 && /\s/.test(triggerText[0]) ? triggerText[0] : "";
      const before = inputText.value.slice(0, matchIndex) + leadingWhitespace;
      const after = inputText.value.slice(cursorPos);
      inputText.value = before + `@${path} ` + after;
    }
    if (workspacePath) {
      const resolved = path.startsWith("/") ? path : `${workspacePath.replace(/\/$/, "")}/${path}`;
      if (!attachments.value.some((a) => a.path === resolved)) {
        addFilePaths([resolved]);
      }
    }
    closePalettes();
  }

  function addFilePaths(paths: string[]) {
    const seenPaths = new Set(attachments.value.map((attachment) => attachment.path));
    const nextAttachments = [...attachments.value];

    for (const filePath of paths) {
      if (!filePath) continue;
      if (seenPaths.has(filePath)) continue;
      seenPaths.add(filePath);

      const name = filePath.split(/[\\/]/).pop() || filePath;
      const ext = name.split(".").pop()?.toLowerCase() || "";
      nextAttachments.push({
        id: createAttachmentId(),
        path: filePath,
        name,
        mimeType: mimeFromExtension(ext)
      });
    }

    attachments.value = nextAttachments;
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

  function cloneAttachments(source: Attachment[]): Attachment[] {
    return source.map((attachment) => ({ ...attachment }));
  }

  function attachmentPayload(source: Attachment[]) {
    return source.map((a) => ({
      path: a.path,
      name: a.name,
      mime_type: a.mimeType
    }));
  }

  async function clearCurrentDraft(extraDraftKey?: string | null) {
    const draftKey = currentDraftKey();
    if (draftKey) {
      await draftStore.clearDraft(draftKey);
    }
    if (extraDraftKey && extraDraftKey !== draftKey) {
      await draftStore.clearDraft(extraDraftKey);
    }
  }

  async function clearComposer(extraDraftKey?: string | null) {
    inputText.value = "";
    attachments.value = [];
    await clearCurrentDraft(extraDraftKey);
  }

  async function invokeSend(content: string, attachmentsToSend: Attachment[]) {
    if (!session.currentSessionId) {
      const draftKeyBeforeMaterialization = currentDraftKey();
      skipNextDraftLoad = true;
      skipNextDraftSaveKey = draftKeyBeforeMaterialization;
      try {
        await session.ensureSessionForSend?.();
      } catch (error) {
        skipNextDraftLoad = false;
        skipNextDraftSaveKey = null;
        throw error;
      }
      if (!session.currentSessionId) {
        skipNextDraftLoad = false;
        skipNextDraftSaveKey = null;
      }
      if (
        draftKeyBeforeMaterialization &&
        skipNextDraftSaveKey === draftKeyBeforeMaterialization &&
        currentDraftKey() === draftKeyBeforeMaterialization
      ) {
        skipNextDraftSaveKey = null;
      }
    }
    if (!session.currentSessionId) {
      throw new Error("No active session");
    }
    await invokeFn("send_message", {
      content,
      attachments: attachmentPayload(attachmentsToSend)
    });
    await session.refreshCurrentSessionMetadata?.(content);
  }

  async function enqueueMessage(content: string, attachmentsToQueue: Attachment[]) {
    queuedMessages.value = [
      ...queuedMessages.value,
      {
        id: createAttachmentId(),
        content,
        attachments: cloneAttachments(attachmentsToQueue)
      }
    ];
    await clearComposer();
  }

  async function sendMessage() {
    const draftAtSend = inputText.value;
    const draftKeyAtSend = currentDraftKey();
    const attachmentsAtSend = attachments.value;
    const content = draftAtSend.trim();
    if (!content && attachmentsAtSend.length === 0) return;
    clearDraftTimer();

    if (isQueueing.value) {
      await enqueueMessage(content, attachmentsAtSend);
      return;
    }

    try {
      await invokeSend(content, attachmentsAtSend);
      const attachmentsUnchanged = attachments.value === attachmentsAtSend;
      const composerUnchanged = inputText.value === draftAtSend && attachmentsUnchanged;
      if (composerUnchanged) {
        await clearComposer(draftKeyAtSend);
      } else if (attachmentsUnchanged && inputText.value === "") {
        attachments.value = [];
        await clearCurrentDraft(draftKeyAtSend);
      } else if (draftKeyAtSend?.startsWith("new-session:")) {
        await draftStore.clearDraft(draftKeyAtSend);
      }
    } catch (e) {
      console.error("Failed to send message:", e);
      if (draftKeyAtSend && inputText.value === draftAtSend) {
        await draftStore.saveDraft(draftKeyAtSend, draftAtSend);
      }
      session.reportSendError?.(String(e));
      notify("error", t("chat.sendFailed", { error: String(e) }));
    }
  }

  async function sendQueuedMessageNow(id: string) {
    if (sendingQueuedId.value) return;
    const queued = queuedMessages.value.find((message) => message.id === id);
    if (!queued) return;

    sendingQueuedId.value = id;
    try {
      await invokeSend(queued.content, queued.attachments);
      queuedMessages.value = queuedMessages.value.filter((message) => message.id !== id);
    } catch (e) {
      console.error("Failed to send queued message:", e);
      session.reportSendError?.(String(e));
      notify("error", t("chat.sendFailed", { error: String(e) }));
    } finally {
      sendingQueuedId.value = null;
    }
  }

  async function sendNextQueuedMessage() {
    if (isQueueing.value || sendingQueuedId.value || queuedMessages.value.length === 0) return;
    await sendQueuedMessageNow(queuedMessages.value[0].id);
  }

  function deleteQueuedMessage(id: string) {
    queuedMessages.value = queuedMessages.value.filter((message) => message.id !== id);
  }

  function clearQueuedMessages() {
    queuedMessages.value = [];
  }

  function moveQueuedMessage(id: string, targetIndex: number): boolean {
    const currentIndex = queuedMessages.value.findIndex((message) => message.id === id);
    if (currentIndex === -1 || targetIndex < 0 || targetIndex >= queuedMessages.value.length) {
      return false;
    }
    if (currentIndex === targetIndex) return true;

    const nextMessages = [...queuedMessages.value];
    const [moved] = nextMessages.splice(currentIndex, 1);
    nextMessages.splice(targetIndex, 0, moved);
    queuedMessages.value = nextMessages;
    return true;
  }

  function restoreQueuedMessage(id: string): boolean {
    const index = queuedMessages.value.findIndex((message) => message.id === id);
    if (index === -1) return false;

    const [queued] = queuedMessages.value.splice(index, 1);
    queuedMessages.value = [...queuedMessages.value];
    inputText.value = queued.content;
    attachments.value = cloneAttachments(queued.attachments);
    closePalettes();
    return true;
  }

  function restoreLastQueuedMessage(): boolean {
    const last = queuedMessages.value.at(-1);
    return last ? restoreQueuedMessage(last.id) : false;
  }

  async function cancelSession() {
    try {
      await invokeFn("cancel_session");
    } catch (e) {
      console.error("Failed to cancel session:", e);
      notify("error", t("chat.cancelFailed", { error: String(e) }));
    }
  }

  async function selectModelProfile(
    alias: string,
    modelPopoverOpen: Ref<boolean>,
    reasoningEffort?: string
  ) {
    if (switchingModel.value) return;
    if (
      alias === session.currentProfile &&
      (reasoningEffort === undefined || reasoningEffort === session.currentReasoningEffort)
    ) {
      modelPopoverOpen.value = false;
      return;
    }
    if (!session.currentSessionId) {
      if (session.setPendingModelSelection) {
        session.setPendingModelSelection(alias, reasoningEffort ?? null);
      } else {
        session.currentProfile = alias;
        session.currentReasoningEffort = reasoningEffort ?? null;
      }
      modelPopoverOpen.value = false;
      return;
    }

    switchingModel.value = true;
    try {
      const payload: {
        sessionId: string;
        profileAlias: string;
        reasoningEffort?: string;
      } = {
        sessionId: session.currentSessionId,
        profileAlias: alias
      };
      if (reasoningEffort !== undefined) {
        payload.reasoningEffort = reasoningEffort;
      }
      await invokeFn("switch_model", payload);
      session.currentProfile = alias;
      session.currentReasoningEffort = reasoningEffort ?? null;
      session.updateSessionProfile?.(session.currentSessionId, alias);
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
    queuedMessages,
    sendingQueuedId,
    switchingModel,
    isQueueing,
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
    sendQueuedMessageNow,
    sendNextQueuedMessage,
    deleteQueuedMessage,
    clearQueuedMessages,
    moveQueuedMessage,
    restoreQueuedMessage,
    restoreLastQueuedMessage,
    cancelSession,
    selectModelProfile,
    appendText
  };

  /** Append text to the composer. If there is already text, add a newline before appending. */
  function appendText(text: string) {
    if (inputText.value.trim()) {
      inputText.value = inputText.value + "\n" + text;
    } else {
      inputText.value = text;
    }
  }
}

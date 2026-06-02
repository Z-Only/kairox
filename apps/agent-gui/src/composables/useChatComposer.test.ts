import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { reactive } from "vue";
import { useChatComposer, type ChatComposerSession } from "./useChatComposer";

function createSession(overrides: Partial<ChatComposerSession> = {}): ChatComposerSession {
  return reactive({
    currentSessionId: "ses_1",
    currentProfile: "fast",
    currentReasoningEffort: null,
    isStreaming: false,
    compacting: false,
    ...overrides
  }) as ChatComposerSession;
}

function createComposer(overrides: Partial<Parameters<typeof useChatComposer>[0]> = {}) {
  const invokeFn = vi.fn(async () => undefined);
  const draftStore = {
    loadDraft: vi.fn(async () => ""),
    saveDraft: vi.fn(async () => undefined),
    clearDraft: vi.fn(async () => undefined)
  };
  const notify = vi.fn();
  const composer = useChatComposer({
    session: createSession(),
    draftStore,
    invokeFn,
    notify,
    t: (key: string, values?: Record<string, unknown>) =>
      values?.error ? `${key}: ${values.error}` : key,
    ...overrides
  });

  return { composer, invokeFn, draftStore, notify };
}

describe("useChatComposer", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("detects slash and file mention triggers from the textarea cursor position", () => {
    const { composer } = createComposer();

    composer.updatePaletteForCursor("  /he", 5);
    expect(composer.paletteFilter.value).toBe("he");
    expect(composer.showCommandPalette.value).toBe(true);
    expect(composer.showMentionPalette.value).toBe(false);

    composer.updatePaletteForCursor("ask @src/com", 12);
    expect(composer.paletteFilter.value).toBe("src/com");
    expect(composer.showCommandPalette.value).toBe(false);
    expect(composer.showMentionPalette.value).toBe(true);

    composer.updatePaletteForCursor("plain text", 10);
    expect(composer.showCommandPalette.value).toBe(false);
    expect(composer.showMentionPalette.value).toBe(false);
  });

  it("replaces active command, skill, and file mention triggers in the draft", () => {
    const { composer } = createComposer();

    composer.inputText.value = " /he";
    composer.onSelectCommand({
      id: "help",
      label: "/help",
      description: "Help",
      insertText: "/help"
    });
    expect(composer.inputText.value).toBe("/help");

    composer.inputText.value = "/sk";
    composer.onSelectSkill("rust");
    expect(composer.inputText.value).toBe("/skills rust ");

    composer.inputText.value = "open @src/mai";
    composer.onSelectFile("src/main.rs");

    // without workspacePath: no attachment added
    expect(composer.attachments.value).toEqual([]);
    expect(composer.inputText.value).toBe("open @src/main.rs ");

    // with workspacePath: adds resolved path as attachment
    composer.inputText.value = "check @src/li";
    composer.onSelectFile("src/lib.rs", "/repo");
    expect(composer.inputText.value).toBe("check @src/lib.rs ");
    expect(composer.attachments.value).toHaveLength(1);
    expect(composer.attachments.value[0].path).toBe("/repo/src/lib.rs");
    expect(composer.attachments.value[0].name).toBe("lib.rs");
    expect(composer.attachments.value[0].mimeType).toBe("text/x-rust");

    // workspacePath trailing slash is normalized
    composer.attachments.value = [];
    composer.inputText.value = "see @docs/READ";
    composer.onSelectFile("docs/README.md", "/repo/");
    expect(composer.attachments.value[0].path).toBe("/repo/docs/README.md");

    // absolute paths pass through unchanged
    composer.attachments.value = [];
    composer.inputText.value = "show @";
    composer.onSelectFile("/abs/path/file.txt", "/repo");
    expect(composer.attachments.value[0].path).toBe("/abs/path/file.txt");

    // dedup: same file mentioned twice only attaches once
    composer.attachments.value = [];
    composer.inputText.value = "look @src/li";
    composer.onSelectFile("src/lib.rs", "/repo");
    expect(composer.attachments.value).toHaveLength(1);
    composer.inputText.value = "also @src/li";
    composer.onSelectFile("src/lib.rs", "/repo");
    expect(composer.attachments.value).toHaveLength(1);
  });

  it("deduplicates selected file attachments while preserving the first occurrence", async () => {
    const openFileDialog = vi.fn(async () => [
      "/repo/notes.md",
      "/repo/src/main.rs",
      "/repo/notes.md"
    ]);
    const { composer } = createComposer({ openFileDialog });

    composer.addFilePaths(["/repo/src/main.rs"]);
    await composer.pickFiles();

    expect(composer.attachments.value.map((attachment) => attachment.path)).toEqual([
      "/repo/src/main.rs",
      "/repo/notes.md"
    ]);
    expect(composer.attachments.value.map((attachment) => attachment.name)).toEqual([
      "main.rs",
      "notes.md"
    ]);
  });

  it("sends trimmed content with attachments then clears composer state and draft", async () => {
    const { composer, invokeFn, draftStore } = createComposer();
    composer.inputText.value = "  hello  ";
    composer.addFilePaths(["/repo/src/main.rs"]);

    await composer.sendMessage();

    expect(invokeFn).toHaveBeenCalledWith("send_message", {
      content: "hello",
      attachments: [{ path: "/repo/src/main.rs", name: "main.rs", mime_type: "text/x-rust" }]
    });
    expect(composer.inputText.value).toBe("");
    expect(composer.attachments.value).toEqual([]);
    expect(draftStore.clearDraft).toHaveBeenCalledWith("ses_1");
  });

  it("preserves draft and attachments when sending fails before IPC accepts the message", async () => {
    const reportSendError = vi.fn();
    const session = createSession({ reportSendError });
    const invokeFn = vi.fn(async () => {
      throw new Error("IPC offline");
    });
    const { composer, draftStore, notify } = createComposer({ session, invokeFn });

    composer.inputText.value = "  keep this draft  ";
    composer.addFilePaths(["/repo/src/main.rs", "/repo/notes.md"]);

    await composer.sendMessage();

    expect(invokeFn).toHaveBeenCalledWith("send_message", {
      content: "keep this draft",
      attachments: [
        { path: "/repo/src/main.rs", name: "main.rs", mime_type: "text/x-rust" },
        { path: "/repo/notes.md", name: "notes.md", mime_type: "text/markdown" }
      ]
    });
    expect(composer.inputText.value).toBe("  keep this draft  ");
    expect(composer.attachments.value.map((att) => att.name)).toEqual(["main.rs", "notes.md"]);
    expect(draftStore.clearDraft).not.toHaveBeenCalled();
    expect(reportSendError).toHaveBeenCalledWith("Error: IPC offline");
    expect(notify).toHaveBeenCalledWith("error", "chat.sendFailed: Error: IPC offline");
  });

  it("queues messages submitted while the session is streaming", async () => {
    const session = createSession({ isStreaming: true });
    const { composer, invokeFn, draftStore } = createComposer({ session });

    composer.inputText.value = "  follow up  ";
    composer.addFilePaths(["/repo/src/main.rs"]);

    await composer.sendMessage();

    expect(invokeFn).not.toHaveBeenCalled();
    expect(composer.queuedMessages.value).toHaveLength(1);
    expect(composer.queuedMessages.value[0].content).toBe("follow up");
    expect(composer.queuedMessages.value[0].attachments[0].name).toBe("main.rs");
    expect(composer.inputText.value).toBe("");
    expect(composer.attachments.value).toEqual([]);
    expect(draftStore.clearDraft).toHaveBeenCalledWith("ses_1");
  });

  it("queues messages submitted while context compaction is running", async () => {
    const session = createSession({ compacting: true });
    const { composer, invokeFn, draftStore } = createComposer({ session });

    composer.inputText.value = "  compacting follow up  ";

    await composer.sendMessage();

    expect(invokeFn).not.toHaveBeenCalled();
    expect(composer.queuedMessages.value).toHaveLength(1);
    expect(composer.queuedMessages.value[0].content).toBe("compacting follow up");
    expect(composer.inputText.value).toBe("");
    expect(draftStore.clearDraft).toHaveBeenCalledWith("ses_1");
  });

  it("does not cap queued messages or report a queue-full error", async () => {
    const session = createSession({ isStreaming: true });
    const { composer, notify } = createComposer({ session });

    for (let i = 1; i <= 15; i += 1) {
      composer.inputText.value = `queued ${i}`;
      await composer.sendMessage();
    }

    expect(composer.queuedMessages.value).toHaveLength(15);
    expect(composer.queuedMessages.value.at(-1)?.content).toBe("queued 15");
    expect(composer.inputText.value).toBe("");
    expect(notify).not.toHaveBeenCalled();
  });

  it("reorders queued messages by moving one item to a target index", async () => {
    const session = createSession({ isStreaming: true });
    const { composer } = createComposer({ session });

    for (const content of ["first", "second", "third"]) {
      composer.inputText.value = content;
      await composer.sendMessage();
    }

    const thirdId = composer.queuedMessages.value[2].id;
    composer.moveQueuedMessage(thirdId, 0);

    expect(composer.queuedMessages.value.map((msg) => msg.content)).toEqual([
      "third",
      "first",
      "second"
    ]);
  });

  it("auto-sends the oldest queued message when streaming stops", async () => {
    const session = createSession({ isStreaming: true });
    const { composer, invokeFn } = createComposer({ session });

    composer.inputText.value = "first queued";
    await composer.sendMessage();
    composer.inputText.value = "second queued";
    await composer.sendMessage();

    session.isStreaming = false;
    await vi.runAllTimersAsync();

    expect(invokeFn).toHaveBeenCalledTimes(1);
    expect(invokeFn).toHaveBeenCalledWith("send_message", {
      content: "first queued",
      attachments: []
    });
    expect(composer.queuedMessages.value.map((msg) => msg.content)).toEqual(["second queued"]);
  });

  it("waits for both streaming and compaction to finish before auto-sending queued messages", async () => {
    const session = createSession({ compacting: true });
    const { composer, invokeFn } = createComposer({ session });

    composer.inputText.value = "queued during compaction";
    await composer.sendMessage();

    session.isStreaming = false;
    await vi.runAllTimersAsync();
    expect(invokeFn).not.toHaveBeenCalled();

    session.compacting = false;
    await vi.runAllTimersAsync();

    expect(invokeFn).toHaveBeenCalledWith("send_message", {
      content: "queued during compaction",
      attachments: []
    });
    expect(composer.queuedMessages.value).toEqual([]);
  });

  it("can guide-send a queued message immediately while streaming", async () => {
    const session = createSession({ isStreaming: true });
    const { composer, invokeFn } = createComposer({ session });

    composer.inputText.value = "correction";
    await composer.sendMessage();
    await composer.sendQueuedMessageNow(composer.queuedMessages.value[0].id);

    expect(invokeFn).toHaveBeenCalledWith("send_message", {
      content: "correction",
      attachments: []
    });
    expect(composer.queuedMessages.value).toEqual([]);
  });

  it("restores the newest queued message into the composer for editing", async () => {
    const session = createSession({ isStreaming: true });
    const { composer } = createComposer({ session });

    composer.inputText.value = "first";
    await composer.sendMessage();
    composer.inputText.value = "second";
    composer.addFilePaths(["/repo/notes.md"]);
    await composer.sendMessage();

    expect(composer.restoreLastQueuedMessage()).toBe(true);

    expect(composer.inputText.value).toBe("second");
    expect(composer.attachments.value[0].name).toBe("notes.md");
    expect(composer.queuedMessages.value.map((msg) => msg.content)).toEqual(["first"]);
  });

  it("restores a selected queued message into the composer for editing", async () => {
    const session = createSession({ isStreaming: true });
    const { composer } = createComposer({ session });

    composer.inputText.value = "first";
    await composer.sendMessage();
    composer.inputText.value = "second";
    await composer.sendMessage();

    const firstId = composer.queuedMessages.value[0].id;
    expect(composer.restoreQueuedMessage(firstId)).toBe(true);

    expect(composer.inputText.value).toBe("first");
    expect(composer.queuedMessages.value.map((msg) => msg.content)).toEqual(["second"]);
  });

  it("deletes queued messages without changing the current draft", async () => {
    const session = createSession({ isStreaming: true });
    const { composer } = createComposer({ session });

    composer.inputText.value = "queued";
    await composer.sendMessage();
    composer.inputText.value = "current draft";

    composer.deleteQueuedMessage(composer.queuedMessages.value[0].id);

    expect(composer.queuedMessages.value).toEqual([]);
    expect(composer.inputText.value).toBe("current draft");
  });

  it("clears all queued messages without sending them", async () => {
    const session = createSession({ isStreaming: true });
    const { composer, invokeFn } = createComposer({ session });

    for (const content of ["first", "second"]) {
      composer.inputText.value = content;
      await composer.sendMessage();
    }

    composer.clearQueuedMessages();

    expect(composer.queuedMessages.value).toEqual([]);
    expect(invokeFn).not.toHaveBeenCalledWith("send_message", expect.anything());
  });

  it("saves the outgoing session draft before loading the next session draft", async () => {
    const session = createSession();
    const { composer, draftStore } = createComposer({ session });

    composer.inputText.value = "draft before switch";
    session.currentSessionId = "ses_2";
    await vi.runAllTimersAsync();

    expect(draftStore.saveDraft).toHaveBeenCalledWith("ses_1", "draft before switch");
    expect(draftStore.loadDraft).toHaveBeenCalledWith("ses_2");
  });

  it("loads and saves placeholder drafts by composer draft key", async () => {
    const session = createSession({
      currentSessionId: null,
      composerDraftKey: null
    });
    const { composer, draftStore } = createComposer({ session });
    draftStore.loadDraft.mockResolvedValueOnce("ordinary cached draft");

    session.composerDraftKey = "new-session:ordinary";
    await vi.runAllTimersAsync();

    expect(draftStore.loadDraft).toHaveBeenCalledWith("new-session:ordinary");
    expect(composer.inputText.value).toBe("ordinary cached draft");

    composer.inputText.value = "ordinary cached draft edited";
    session.composerDraftKey = "new-session:project:p1";
    await vi.runAllTimersAsync();

    expect(draftStore.saveDraft).toHaveBeenCalledWith(
      "new-session:ordinary",
      "ordinary cached draft edited"
    );
    expect(draftStore.loadDraft).toHaveBeenCalledWith("new-session:project:p1");
  });

  it("materializes placeholder sessions before sending the first message", async () => {
    const session = createSession({
      currentSessionId: null,
      composerDraftKey: "new-session:ordinary",
      ensureSessionForSend: vi.fn(async () => {
        session.currentSessionId = "ses_new";
        session.composerDraftKey = "ses_new";
      })
    });
    const { composer, invokeFn } = createComposer({ session });
    composer.inputText.value = "hello from placeholder";

    await composer.sendMessage();

    expect(session.ensureSessionForSend).toHaveBeenCalled();
    expect(invokeFn).toHaveBeenCalledWith("send_message", {
      content: "hello from placeholder",
      attachments: []
    });
  });

  it("refreshes current session metadata after a placeholder send succeeds", async () => {
    const refreshCurrentSessionMetadata = vi.fn(async () => undefined);
    const session = createSession({
      currentSessionId: null,
      composerDraftKey: "new-session:project:p1",
      ensureSessionForSend: vi.fn(async () => {
        session.currentSessionId = "ses_new";
        session.composerDraftKey = "ses_new";
      })
    });
    (
      session as ChatComposerSession & {
        refreshCurrentSessionMetadata: (firstMessageContent?: string) => Promise<void>;
      }
    ).refreshCurrentSessionMetadata = refreshCurrentSessionMetadata;
    const { composer, invokeFn } = createComposer({ session });
    composer.inputText.value = "hello from project placeholder";

    await composer.sendMessage();

    expect(invokeFn).toHaveBeenCalledWith("send_message", {
      content: "hello from project placeholder",
      attachments: []
    });
    expect(refreshCurrentSessionMetadata).toHaveBeenCalledTimes(1);
    expect(refreshCurrentSessionMetadata).toHaveBeenCalledWith("hello from project placeholder");
    expect(refreshCurrentSessionMetadata.mock.invocationCallOrder[0]).toBeGreaterThan(
      invokeFn.mock.invocationCallOrder[0]
    );
  });

  it("clears sent attachments after materializing a placeholder session", async () => {
    const session = createSession({
      currentSessionId: null,
      composerDraftKey: "new-session:project:p1",
      ensureSessionForSend: vi.fn(async () => {
        session.currentSessionId = "ses_new";
        session.composerDraftKey = "ses_new";
      })
    });
    let composerRef: ReturnType<typeof useChatComposer> | null = null;
    const invokeFn = vi.fn(async () => {
      // Mirrors the live placeholder materialization path where the draft
      // watcher can clear the textarea before the accepted send returns.
      if (composerRef) composerRef.inputText.value = "";
    });
    const { composer } = createComposer({ session, invokeFn });
    composerRef = composer;
    composer.inputText.value = "read this";
    composer.addFilePaths(["/repo/docs/notes.md"]);

    await composer.sendMessage();
    await vi.runAllTimersAsync();

    expect(invokeFn).toHaveBeenCalledWith("send_message", {
      content: "read this",
      attachments: [{ path: "/repo/docs/notes.md", name: "notes.md", mime_type: "text/markdown" }]
    });
    expect(composer.inputText.value).toBe("");
    expect(composer.attachments.value).toEqual([]);
  });

  it("keeps placeholder text when first send fails after materialization", async () => {
    const session = createSession({
      currentSessionId: null,
      composerDraftKey: "new-session:ordinary",
      ensureSessionForSend: vi.fn(async () => {
        session.currentSessionId = "ses_new";
        session.composerDraftKey = "ses_new";
      })
    });
    const { composer, invokeFn } = createComposer({ session });
    invokeFn.mockRejectedValueOnce(new Error("model offline"));
    composer.inputText.value = "keep this draft";

    await composer.sendMessage();
    await vi.runAllTimersAsync();

    expect(composer.inputText.value).toBe("keep this draft");
  });

  it("switches models with a selected reasoning effort", async () => {
    const session = createSession();
    const modelPopoverOpen = { value: true };
    const { composer, invokeFn } = createComposer({ session });

    await composer.selectModelProfile("smart", modelPopoverOpen, "xhigh");

    expect(invokeFn).toHaveBeenCalledWith("switch_model", {
      sessionId: "ses_1",
      profileAlias: "smart",
      reasoningEffort: "xhigh"
    });
    expect(session.currentProfile).toBe("smart");
    expect(session.currentReasoningEffort).toBe("xhigh");
    expect(modelPopoverOpen.value).toBe(false);
  });

  it("updates the pending session model without IPC before first send", async () => {
    const session = createSession({ currentSessionId: null });
    const modelPopoverOpen = { value: true };
    const { composer, invokeFn } = createComposer({ session });

    await composer.selectModelProfile("smart", modelPopoverOpen, "xhigh");

    expect(invokeFn).not.toHaveBeenCalledWith("switch_model", expect.anything());
    expect(session.currentProfile).toBe("smart");
    expect(session.currentReasoningEffort).toBe("xhigh");
    expect(modelPopoverOpen.value).toBe(false);
  });

  it("allows changing reasoning effort without changing the model alias", async () => {
    const session = createSession({ currentProfile: "smart", currentReasoningEffort: "low" });
    const modelPopoverOpen = { value: true };
    const { composer, invokeFn } = createComposer({ session });

    await composer.selectModelProfile("smart", modelPopoverOpen, "high");

    expect(invokeFn).toHaveBeenCalledWith("switch_model", {
      sessionId: "ses_1",
      profileAlias: "smart",
      reasoningEffort: "high"
    });
    expect(session.currentProfile).toBe("smart");
    expect(session.currentReasoningEffort).toBe("high");
    expect(modelPopoverOpen.value).toBe(false);
  });
});

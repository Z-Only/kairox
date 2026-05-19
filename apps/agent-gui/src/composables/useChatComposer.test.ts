import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { reactive } from "vue";
import { useChatComposer, type ChatComposerSession } from "./useChatComposer";

function createSession(overrides: Partial<ChatComposerSession> = {}): ChatComposerSession {
  return reactive({
    currentSessionId: "ses_1",
    currentProfile: "fast",
    currentReasoningEffort: null,
    isStreaming: false,
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
    expect(composer.inputText.value).toBe("open@src/main.rs ");

    // without workspacePath: no attachment added
    expect(composer.attachments.value).toEqual([]);

    // with workspacePath: adds resolved path as attachment
    composer.inputText.value = "check @src/li";
    composer.onSelectFile("src/lib.rs", "/repo");
    expect(composer.inputText.value).toBe("check@src/lib.rs ");
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

  it("caps queued messages and reports when the queue is full", async () => {
    const session = createSession({ isStreaming: true });
    const { composer, notify } = createComposer({ session });

    for (let i = 1; i <= 10; i += 1) {
      composer.inputText.value = `queued ${i}`;
      await composer.sendMessage();
    }
    composer.inputText.value = "queued 11";

    await composer.sendMessage();

    expect(composer.queuedMessages.value.map((msg) => msg.content)).toEqual([
      "queued 1",
      "queued 2",
      "queued 3",
      "queued 4",
      "queued 5",
      "queued 6",
      "queued 7",
      "queued 8",
      "queued 9",
      "queued 10"
    ]);
    expect(composer.inputText.value).toBe("queued 11");
    expect(notify).toHaveBeenCalledWith("error", "chat.queueFull");
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

  it("saves the outgoing session draft before loading the next session draft", async () => {
    const session = createSession();
    const { composer, draftStore } = createComposer({ session });

    composer.inputText.value = "draft before switch";
    session.currentSessionId = "ses_2";
    await vi.runAllTimersAsync();

    expect(draftStore.saveDraft).toHaveBeenCalledWith("ses_1", "draft before switch");
    expect(draftStore.loadDraft).toHaveBeenCalledWith("ses_2");
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

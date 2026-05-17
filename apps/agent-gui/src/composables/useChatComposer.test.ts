import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { reactive } from "vue";
import { useChatComposer, type ChatComposerSession } from "./useChatComposer";

function createSession(overrides: Partial<ChatComposerSession> = {}): ChatComposerSession {
  return reactive({
    currentSessionId: "ses_1",
    currentProfile: "fast",
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

  it("saves the outgoing session draft before loading the next session draft", async () => {
    const session = createSession();
    const { composer, draftStore } = createComposer({ session });

    composer.inputText.value = "draft before switch";
    session.currentSessionId = "ses_2";
    await vi.runAllTimersAsync();

    expect(draftStore.saveDraft).toHaveBeenCalledWith("ses_1", "draft before switch");
    expect(draftStore.loadDraft).toHaveBeenCalledWith("ses_2");
  });
});

import { describe, it, expect } from "vitest";
import type { CompactionStatus } from "@/types";
import type { TraceEntryData, TraceEntryKind } from "@/types/trace";
import { buildChatStream, type ChatStreamMessageInput } from "./useChatStream";
import type {
  ChatCompactionStreamItem,
  ChatMessageStreamItem,
  ChatPermissionStreamItem,
  ChatToolCallStreamItem
} from "@/types/chatStream";

const idle: CompactionStatus = { type: "Idle" };

function toolEntry(overrides: Partial<TraceEntryData> & { id: string }): TraceEntryData {
  return {
    kind: "tool",
    status: "completed",
    title: "tool",
    startedAt: 0,
    expanded: false,
    ...overrides
  };
}

function permissionEntry(overrides: Partial<TraceEntryData> & { id: string }): TraceEntryData {
  return {
    kind: "permission",
    status: "pending",
    title: "perm",
    startedAt: 0,
    expanded: true,
    ...overrides
  };
}

function memoryEntry(overrides: Partial<TraceEntryData> & { id: string }): TraceEntryData {
  return {
    kind: "memory",
    status: "pending",
    title: "mem",
    startedAt: 0,
    expanded: true,
    ...overrides
  };
}

describe("buildChatStream", () => {
  it("returns an empty list when there are no messages, no trace entries, and compaction is Idle", () => {
    const result = buildChatStream([], [], idle);
    expect(result).toEqual([]);
  });

  it("emits one ChatMessageStreamItem per message in projection order with stable msg-<index> ids", () => {
    const messages: ChatStreamMessageInput[] = [
      { role: "user", content: "hello" },
      { role: "assistant", content: "hi there", sourceAgentId: "worker-1" }
    ];

    const result = buildChatStream(messages, [], idle);

    expect(result).toHaveLength(2);
    expect(result[0]).toEqual<ChatMessageStreamItem>({
      kind: "message",
      id: "msg-0",
      role: "user",
      content: "hello"
    });
    expect(result[1]).toEqual<ChatMessageStreamItem>({
      kind: "message",
      id: "msg-1",
      role: "assistant",
      content: "hi there",
      sourceAgentId: "worker-1"
    });
  });

  it("maps tool / permission / memory trace entries to their typed stream items", () => {
    const entries: TraceEntryData[] = [
      toolEntry({
        id: "tool-1",
        toolId: "shell",
        title: "shell exec",
        status: "running",
        durationMs: 42,
        input: "echo hi",
        outputPreview: "hi",
        scope: "session",
        startedAt: 1
      }),
      permissionEntry({
        id: "perm-1",
        toolId: "fs.write",
        title: "Allow write?",
        input: "/tmp/x",
        scope: "session",
        rawEvent: '{"type":"PermissionRequested"}',
        startedAt: 2
      }),
      memoryEntry({
        id: "mem-1",
        scope: "workspace",
        content: "remember this",
        reason: "user request",
        title: "Save workspace memory",
        startedAt: 3
      })
    ];

    const result = buildChatStream([], entries, idle);

    expect(result).toHaveLength(3);

    expect(result[0]).toEqual<ChatToolCallStreamItem>({
      kind: "tool_call",
      id: "tool-1",
      toolId: "shell",
      title: "shell exec",
      status: "running",
      durationMs: 42,
      input: "echo hi",
      outputPreview: "hi",
      scope: "session"
    });

    expect(result[1]).toEqual<ChatPermissionStreamItem>({
      kind: "permission",
      id: "perm-1",
      variant: "tool",
      toolId: "fs.write",
      title: "Allow write?",
      input: "/tmp/x",
      scope: "session",
      rawEvent: '{"type":"PermissionRequested"}'
    });

    expect(result[2]).toEqual<ChatPermissionStreamItem>({
      kind: "permission",
      id: "mem-1",
      variant: "memory",
      title: "Save workspace memory",
      scope: "workspace",
      content: "remember this",
      reason: "user request"
    });
  });

  it("skips trace entries whose kind is not tool / permission / memory", () => {
    const unknownEntry: TraceEntryData = {
      id: "unk-1",
      // Force an unknown kind for the defensive default branch.
      kind: "unknown" as unknown as TraceEntryKind,
      status: "completed",
      title: "?",
      startedAt: 0,
      expanded: false
    };

    const result = buildChatStream([], [unknownEntry], idle);
    expect(result).toEqual([]);
  });

  it("does not append a compaction item when compaction.type === 'Idle'", () => {
    const result = buildChatStream([{ role: "user", content: "x" }], [], idle);
    expect(result.some((item) => item.kind === "compaction")).toBe(false);
  });

  it("appends exactly one ChatCompactionStreamItem at the end when compaction.type === 'Running'", () => {
    const running: CompactionStatus = { type: "Running" };
    const messages: ChatStreamMessageInput[] = [{ role: "user", content: "hello" }];
    const entries: TraceEntryData[] = [toolEntry({ id: "tool-1" })];

    const result = buildChatStream(messages, entries, running);

    expect(result).toHaveLength(3);
    const compactionItem = result[result.length - 1];
    expect(compactionItem).toEqual<ChatCompactionStreamItem>({
      kind: "compaction",
      id: "compaction-Running",
      status: running
    });
    expect(result.filter((item) => item.kind === "compaction")).toHaveLength(1);
  });

  it("preserves trace insertion order when entries share equal startedAt values", () => {
    const entries: TraceEntryData[] = [
      toolEntry({ id: "first", title: "first", startedAt: 0 }),
      toolEntry({ id: "second", title: "second", startedAt: 0 }),
      toolEntry({ id: "third", title: "third", startedAt: 0 })
    ];

    const result = buildChatStream([], entries, idle);

    expect(result.map((item) => item.id)).toEqual(["first", "second", "third"]);
  });

  it("orders trace items by startedAt ascending when timestamps differ", () => {
    const entries: TraceEntryData[] = [
      toolEntry({ id: "c", startedAt: 30 }),
      toolEntry({ id: "a", startedAt: 10 }),
      toolEntry({ id: "b", startedAt: 20 })
    ];

    const result = buildChatStream([], entries, idle);

    expect(result.map((item) => item.id)).toEqual(["a", "b", "c"]);
  });

  it("never mutates the input arrays", () => {
    const messages: ChatStreamMessageInput[] = [{ role: "user", content: "hi" }];
    const entries: TraceEntryData[] = [
      toolEntry({ id: "second", startedAt: 20 }),
      toolEntry({ id: "first", startedAt: 10 })
    ];
    const messagesSnapshot = [...messages];
    const entriesSnapshot = [...entries];

    buildChatStream(messages, entries, { type: "Failed", error: "boom" });

    expect(messages).toEqual(messagesSnapshot);
    expect(entries).toEqual(entriesSnapshot);
  });
});
